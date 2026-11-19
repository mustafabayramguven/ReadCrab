#![allow(dead_code)]
// Sahte HTTP sunucusu: çekirdek testleri için tiny_http tabanlı, her isteği kaydeder.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String,
    pub path: String, // sorgu dahil
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn json(status: u16, v: &serde_json::Value) -> Reply {
    Reply {
        status,
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: serde_json::to_vec(v).expect("json"),
    }
}

pub fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Reply {
    Reply {
        status,
        headers: vec![("Content-Type".into(), content_type.into())],
        body,
    }
}

pub struct Mock {
    pub base: String,
    pub log: Arc<Mutex<Vec<Req>>>,
    port: u16,
    stop_flag: Arc<AtomicBool>,
    server: Arc<tiny_http::Server>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Mock {
    pub fn start(handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> Mock {
        Self::start_on(0, handler)
    }

    pub fn start_on(port: u16, handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> Mock {
        let server = Arc::new(tiny_http::Server::http(("127.0.0.1", port)).expect("bind"));
        let port = server.server_addr().to_ip().expect("ip").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let (srv, lg, st) = (server.clone(), log.clone(), stop_flag.clone());
        let worker = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let Ok(Some(mut rq)) = srv.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                let mut body = Vec::new();
                let _ = rq.as_reader().read_to_end(&mut body);
                let req = Req {
                    method: rq.method().as_str().to_string(),
                    path: rq.url().to_string(),
                    headers: rq
                        .headers()
                        .iter()
                        .map(|h| {
                            (
                                h.field.as_str().as_str().to_string(),
                                h.value.as_str().to_string(),
                            )
                        })
                        .collect(),
                    body,
                };
                let reply = handler(&req);
                lg.lock().unwrap().push(req);
                let mut resp = tiny_http::Response::from_data(reply.body)
                    .with_status_code(reply.status);
                for (k, v) in reply.headers {
                    if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
                        resp.add_header(h);
                    }
                }
                let date = httpdate::fmt_http_date(SystemTime::now());
                resp.add_header(
                    tiny_http::Header::from_bytes(&b"Date"[..], date.as_bytes()).unwrap(),
                );
                let _ = rq.respond(resp);
            }
        });

        Mock {
            base: format!("http://127.0.0.1:{port}"),
            log,
            port,
            stop_flag,
            server,
            worker: Some(worker),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    // Dinleyiciyi kapatır (çevrimdışı simülasyonu). tiny_http `Server`'ı bırakınca dinleyici arka
    // planda kapanır; port gerçekten bağlantı reddedene dek beklenir, zamanlayıcıya güvenilmez.
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
        let port = self.port;
        drop(self.server);
        wait_closed(port);
    }

    pub fn requests(&self) -> Vec<Req> {
        self.log.lock().unwrap().clone()
    }
}

// Port bağlantı reddedene dek bekler (en çok 5 sn).
pub fn wait_closed(port: u16) {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub const TOKEN: &str = "testtoken123";

pub fn core_in(dir: &tempfile::TempDir) -> std::sync::Arc<readcrab_core::Core> {
    readcrab_core::Core::new(dir.path().to_string_lossy().into_owned()).unwrap()
}

// Sunucunun gerçek POST /api/login 200 yanıtı: gövde UserRes {id, username, is_admin}, Set-Cookie build_session_cookie biçimi
pub fn login_ok() -> Reply {
    let mut r = json(200, &serde_json::json!({"id": 1, "username": "admin", "is_admin": true}));
    r.headers.push((
        "Set-Cookie".to_string(),
        format!("krs_session={TOKEN}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000"),
    ));
    r
}

// Sunucunun gerçek 401 gövdesi
pub fn login_fail() -> Reply {
    json(401, &serde_json::json!({"error": "unauthorized", "message": "Oturum açmanız gerekiyor."}))
}

pub fn is_login(req: &Req) -> bool {
    req.method == "POST" && req.path == "/api/login"
}

pub fn has_cookie(req: &Req) -> bool {
    req.headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("cookie") && v == &format!("krs_session={TOKEN}"))
}
