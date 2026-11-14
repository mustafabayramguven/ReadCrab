use std::{
    io::Read,
    sync::{
        atomic::{AtomicI64, Ordering},
        Mutex, MutexGuard,
    },
    time::{Duration, UNIX_EPOCH},
};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{error::CoreError, util};

const MAX_BODY: u64 = 64 * 1024 * 1024;
const COOKIE_NAME: &str = "krs_session";

/// İç HTTP hatası (UniFFI'ye açılmaz; `error::into_core` ile çevrilir).
#[derive(Debug)]
pub enum HErr {
    Net(String),
    Status {
        code: u16,
        error: String,
        message: String,
        retry_after: Option<u64>,
    },
    Parse(String),
}

/// Başarılı (2xx) yanıt.
pub struct Reply {
    pub content_type: String,
    pub set_cookies: Vec<String>,
    pub body: Vec<u8>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn no_session() -> HErr {
    HErr::Status {
        code: 401,
        error: "unauthorized".to_string(),
        message: "Oturum açmanız gerekiyor.".to_string(),
        retry_after: None,
    }
}

pub struct Http {
    agent: ureq::Agent,
    base: Mutex<String>,
    token: Mutex<Option<String>>,
    offset: AtomicI64,
}

impl Http {
    pub fn new() -> Http {
        // Bağlantı havuzu kapalı: ureq havuza dönen akışın okuma zaman aşımını sıfırlar (Stream::reset)
        // ve yeniden kullanımda geri koymaz; yanıt vermeyen (yarı ölü) bir akışta istek sonsuza dek asılırdı.
        let agent = ureq::AgentBuilder::new()
            .max_idle_connections(0)
            .timeout_connect(Duration::from_secs(8))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .user_agent("ReadCrab-Android/0.1")
            .build();
        Http {
            agent,
            base: Mutex::new(String::new()),
            token: Mutex::new(None),
            offset: AtomicI64::new(0),
        }
    }

    pub fn set_session(&self, base: &str, token: &str) {
        *lock(&self.base) = base.to_string();
        *lock(&self.token) = Some(token.to_string());
    }

    pub fn clear_session(&self) {
        lock(&self.base).clear();
        *lock(&self.token) = None;
    }

    pub fn clock_offset(&self) -> i64 {
        self.offset.load(Ordering::Relaxed)
    }

    pub fn set_offset(&self, v: i64) {
        self.offset.store(v, Ordering::Relaxed);
    }

    fn note_date(&self, resp: &ureq::Response) {
        if let Some(d) = resp.header("Date") {
            if let Some(o) = offset_from_date(d, util::sys_now()) {
                self.offset.store(o, Ordering::Relaxed);
            }
        }
    }

    fn exec(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
        total: Option<Duration>,
        base: Option<&str>,
    ) -> Result<Reply, HErr> {
        // base verilmişse (giriş) çerez gönderilmez
        let (base, token) = match base {
            Some(b) => (b.to_string(), None),
            None => {
                let t = lock(&self.token).clone().ok_or_else(no_session)?;
                (lock(&self.base).clone(), Some(t))
            }
        };
        if base.is_empty() {
            return Err(no_session());
        }
        let url = format!("{base}{path}");
        let mut req = self.agent.request(method, &url);
        if let Some(t) = total {
            req = req.timeout(t);
        }
        if let Some(t) = token {
            req = req.set("Cookie", &format!("{COOKIE_NAME}={t}"));
        }
        // sunucu POST/PUT/DELETE için HER ZAMAN application/json ister (gövdesiz DELETE dahil)
        if method != "GET" {
            req = req.set("Content-Type", "application/json");
        }
        let res = match body {
            Some(b) => req.send_string(&b),
            None => req.call(),
        };
        match res {
            Ok(resp) => {
                self.note_date(&resp);
                let content_type = resp.content_type().to_string();
                let set_cookies = resp.all("set-cookie").iter().map(|s| s.to_string()).collect();
                let body = read_all(resp, MAX_BODY)?;
                Ok(Reply {
                    content_type,
                    set_cookies,
                    body,
                })
            }
            Err(ureq::Error::Status(code, resp)) => {
                let retry_after = resp
                    .header("Retry-After")
                    .and_then(|v| v.trim().parse::<u64>().ok());
                let body = read_all(resp, 64 * 1024).unwrap_or_default();
                let v: serde_json::Value =
                    serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
                Err(HErr::Status {
                    code,
                    error: v["error"].as_str().unwrap_or("").to_string(),
                    message: v["message"].as_str().unwrap_or("").to_string(),
                    retry_after,
                })
            }
            Err(ureq::Error::Transport(t)) => Err(HErr::Net(t.to_string())),
        }
    }

    pub fn get(&self, path: &str) -> Result<Reply, HErr> {
        self.exec("GET", path, None, None, None)
    }

    pub fn get_timeout(&self, path: &str, total: Duration) -> Result<Reply, HErr> {
        self.exec("GET", path, None, Some(total), None)
    }

    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, HErr> {
        parse_json(&self.get(path)?.body)
    }

    pub fn get_json_timeout<T: DeserializeOwned>(
        &self,
        path: &str,
        total: Duration,
    ) -> Result<T, HErr> {
        parse_json(&self.get_timeout(path, total)?.body)
    }

    /// PUT/POST: gövde JSON.
    pub fn send_json(
        &self,
        method: &str,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<Reply, HErr> {
        self.exec(method, path, Some(body.to_string()), None, None)
    }

    /// Gövdesiz istek (DELETE); `Content-Type: application/json` yine gider.
    pub fn send_empty(&self, method: &str, path: &str) -> Result<Reply, HErr> {
        self.exec(method, path, None, None, None)
    }

    /// `POST /api/login`; (yanıt, oturum belirteci) döner.
    pub fn login(
        &self,
        base: &str,
        username: &str,
        password: &str,
    ) -> Result<(LoginRes, String), HErr> {
        let body = serde_json::json!({ "username": username, "password": password });
        let r = self.exec("POST", "/api/login", Some(body.to_string()), None, Some(base))?;
        let res: LoginRes = parse_json(&r.body)?;
        let headers: Vec<&str> = r.set_cookies.iter().map(|s| s.as_str()).collect();
        let token = parse_session_cookie(&headers)
            .ok_or_else(|| HErr::Parse("oturum çerezi yok".to_string()))?;
        Ok((res, token))
    }
}

fn read_all(resp: ureq::Response, cap: u64) -> Result<Vec<u8>, HErr> {
    let mut buf = Vec::new();
    resp.into_reader()
        .take(cap)
        .read_to_end(&mut buf)
        .map_err(|e| HErr::Net(e.to_string()))?;
    Ok(buf)
}

pub fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, HErr> {
    serde_json::from_slice(body).map_err(|e| HErr::Parse(e.to_string()))
}

/// `Date` başlığından `sunucu_sn - yerel_sn`.
pub fn offset_from_date(date: &str, local_secs: i64) -> Option<i64> {
    let t = httpdate::parse_http_date(date).ok()?;
    let server = t.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(server - local_secs)
}

/// `Set-Cookie` başlık değerlerinden `krs_session` değeri.
pub fn parse_session_cookie(headers: &[&str]) -> Option<String> {
    for h in headers {
        if let Some(rest) = h.trim_start().strip_prefix("krs_session=") {
            let v = rest.split(';').next().unwrap_or("").trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Kırp, `://` yoksa `http://` ekle, sondaki `/` kırp; boşsa `InvalidInput`.
pub fn normalize_url(s: &str) -> Result<String, CoreError> {
    let t = s.trim();
    let with_scheme = if t.contains("://") {
        t.to_string()
    } else {
        format!("http://{t}")
    };
    let out = with_scheme.trim_end_matches('/').to_string();
    let host = out.split_once("://").map(|(_, h)| h).unwrap_or("");
    if t.is_empty() || host.is_empty() {
        return Err(CoreError::InvalidInput("Sunucu adresi gerekli.".to_string()));
    }
    Ok(out)
}

/// Sorgu değeri için yüzde-kodlama.
pub fn encode_component(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

// ---- DTO'lar (sunucu yanıtları) ----

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
pub struct LoginRes {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Deserialize)]
pub struct SeriesPageDto {
    pub total: i64,
    pub items: Vec<SeriesListDto>,
}

#[derive(Deserialize, Clone)]
pub struct SeriesListDto {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub book_count: i64,
    #[serde(default)]
    pub read_count: i64,
    #[serde(default)]
    pub reading_count: i64,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub cover_book_id: Option<i64>,
    #[serde(default)]
    pub cover_v: Option<i64>,
    #[serde(default)]
    pub last_read_at: Option<i64>,
}

#[derive(Deserialize)]
pub struct SeriesDetailDto {
    pub id: i64,
    pub name: String,
    pub books: Vec<BookDto>,
}

#[derive(Deserialize)]
pub struct BookDto {
    pub id: i64,
    pub title: String,
    pub format: String,
    #[serde(default)]
    pub page_count: i64,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub cover_v: i64,
}

#[derive(Deserialize)]
pub struct ProgressDto {
    #[serde(default)]
    pub locator: String,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Deserialize)]
pub struct PutResDto {
    #[serde(default = "default_true")]
    pub applied: bool,
}

#[derive(Deserialize)]
pub struct BookmarkDto {
    pub id: i64,
    pub locator: String,
    pub label: String,
    pub created_at: i64,
}

#[derive(Deserialize)]
pub struct BookmarksDto {
    pub items: Vec<BookmarkDto>,
}

#[derive(Deserialize)]
pub struct PagesDto {
    pub count: i64,
}

#[derive(Deserialize)]
pub struct TocChapterDto {
    pub index: i64,
    pub title: String,
}

#[derive(Deserialize)]
pub struct TocDto {
    pub chapters: Vec<TocChapterDto>,
}

#[derive(Deserialize)]
pub struct SearchHitDto {
    pub ch: i64,
    pub p: f64,
    pub before: String,
    pub hit: String,
    pub after: String,
}

#[derive(Deserialize)]
pub struct SearchDto {
    pub results: Vec<SearchHitDto>,
    #[serde(default)]
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::into_core;

    #[test]
    fn session_cookie_parsing() {
        assert_eq!(
            parse_session_cookie(&["other=1; Path=/", "krs_session=abc123; Path=/; Secure; HttpOnly"]),
            Some("abc123".to_string())
        );
        assert_eq!(parse_session_cookie(&["krs_session=tok"]), Some("tok".to_string()));
        assert_eq!(parse_session_cookie(&["krs_session=; Max-Age=0"]), None);
        assert_eq!(parse_session_cookie(&["x=1"]), None);
        assert_eq!(parse_session_cookie(&[]), None);
    }

    #[test]
    fn date_header_offset() {
        // 1994-11-06 08:49:37 GMT = 784111777
        assert_eq!(
            offset_from_date("Sun, 06 Nov 1994 08:49:37 GMT", 784_111_772),
            Some(5)
        );
        assert_eq!(offset_from_date("bozuk", 1), None);
    }

    #[test]
    fn url_normalization() {
        assert_eq!(
            normalize_url("192.168.1.2:3435").unwrap(),
            "http://192.168.1.2:3435"
        );
        assert_eq!(
            normalize_url("  https://ornek.org/ ").unwrap(),
            "https://ornek.org"
        );
        assert_eq!(normalize_url("http://a:1///").unwrap(), "http://a:1");
        assert!(matches!(normalize_url(""), Err(CoreError::InvalidInput(_))));
        assert!(matches!(normalize_url("   "), Err(CoreError::InvalidInput(_))));
        assert!(matches!(normalize_url("http://"), Err(CoreError::InvalidInput(_))));
    }

    #[test]
    fn herr_mapping() {
        let tls = into_core(HErr::Net(
            "tls connection init failed: invalid peer certificate: UnknownIssuer".to_string(),
        ));
        assert!(matches!(tls, CoreError::Tls(_)));
        let net = into_core(HErr::Net("Connection refused".to_string()));
        assert!(matches!(net, CoreError::Network(_)));
        let st = |code: u16, message: &str| HErr::Status {
            code,
            error: String::new(),
            message: message.to_string(),
            retry_after: None,
        };
        assert!(matches!(into_core(st(401, "x")), CoreError::Unauthorized));
        assert!(matches!(into_core(st(404, "x")), CoreError::NotFound));
        match into_core(st(429, "Çok fazla deneme.")) {
            CoreError::Server(m) => assert_eq!(m, "Çok fazla deneme."),
            _ => panic!("Server bekleniyordu"),
        }
        match into_core(st(500, "")) {
            CoreError::Server(m) => assert_eq!(m, "HTTP 500"),
            _ => panic!("Server bekleniyordu"),
        }
        assert!(matches!(
            into_core(HErr::Parse("x".to_string())),
            CoreError::Server(_)
        ));
    }

    #[test]
    fn encode_component_escapes() {
        assert_eq!(encode_component("a b&ç"), "a%20b%26%C3%A7");
    }
}
