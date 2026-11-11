uniffi::setup_scaffolding!();

mod db;
mod download;
mod error;
mod http;
mod library;
mod reader;
mod sync;
mod util;

pub use error::CoreError;

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
};

use rusqlite::Connection;

use error::into_core;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ReadStatus {
    Unread,
    Reading,
    Finished,
    Dropped,
}

impl ReadStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ReadStatus::Unread => "unread",
            ReadStatus::Reading => "reading",
            ReadStatus::Finished => "finished",
            ReadStatus::Dropped => "dropped",
        }
    }

    pub(crate) fn from_db(s: &str) -> ReadStatus {
        match s {
            "reading" => ReadStatus::Reading,
            "finished" => ReadStatus::Finished,
            "dropped" => ReadStatus::Dropped,
            _ => ReadStatus::Unread,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DownloadState {
    None,
    Queued,
    Running,
    Failed,
    Done,
}

impl DownloadState {
    pub(crate) fn from_db(s: Option<&str>) -> DownloadState {
        match s {
            Some("queued") => DownloadState::Queued,
            Some("running") => DownloadState::Running,
            Some("failed") => DownloadState::Failed,
            Some("done") => DownloadState::Done,
            _ => DownloadState::None,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SessionInfo {
    pub server_url: String,
    pub username: String,
    pub user_id: i64,
    pub is_admin: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DownloadInfo {
    pub book_id: i64,
    pub state: DownloadState,
    pub parts_done: i32,
    pub parts_total: i32,
    pub bytes: i64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SeriesItem {
    pub id: i64,
    pub name: String,
    pub book_count: i32,
    pub read_count: i32,
    pub reading_count: i32,
    pub downloaded_count: i32,
    pub percent: f64,
    pub cover_book_id: Option<i64>,
    pub cover_v: i64,
    pub last_read_at: Option<i64>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ContinueItem {
    pub book_id: i64,
    pub title: String,
    pub series_id: i64,
    pub series_name: String,
    pub is_comic: bool,
    pub percent: f64,
    pub cover_v: i64,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookItem {
    pub id: i64,
    pub series_id: i64,
    pub title: String,
    pub format: String,
    pub is_comic: bool,
    pub page_count: i32,
    pub size: i64,
    pub error: Option<String>,
    pub cover_v: i64,
    pub status: ReadStatus,
    pub percent: f64,
    pub download: DownloadInfo,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SeriesDetail {
    pub id: i64,
    pub name: String,
    pub books: Vec<BookItem>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookDetail {
    pub book: BookItem,
    pub series_name: String,
    pub locator: String,
    pub next_book_id: Option<i64>,
    pub next_book_title: Option<String>,
    pub prev_book_id: Option<i64>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ProgressInfo {
    pub locator: String,
    pub percent: f64,
    pub status: ReadStatus,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct BookmarkItem {
    pub local_id: i64,
    pub locator: String,
    pub label: String,
    pub created_at: i64,
    pub pending: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct TocEntry {
    pub index: i32,
    pub title: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct TextInfo {
    pub chapter_count: i32,
    pub toc: Vec<TocEntry>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchHitItem {
    pub ch: i32,
    pub p: f64,
    pub before: String,
    pub hit: String,
    pub after: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SearchResults {
    pub hits: Vec<SearchHitItem>,
    pub truncated: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncReport {
    pub pushed: i32,
    pub pulled_books: i32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncStatus {
    pub pending_ops: i32,
    pub last_ok_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DownloadRun {
    pub finished: i32,
    pub failed: i32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct StorageUsage {
    pub downloads_bytes: i64,
    pub cache_bytes: i64,
    pub covers_bytes: i64,
    pub db_bytes: i64,
}

#[derive(uniffi::Object)]
pub struct Core {
    pub(crate) db: Mutex<Connection>,
    pub(crate) http: http::Http,
    pub(crate) root: PathBuf,
    pub(crate) stop: AtomicBool,
    pub(crate) cancel: Mutex<HashSet<i64>>,
    pub(crate) dl_running: AtomicBool,
}

impl Core {
    /// Veritabanı kilidini alır (zehirli kilidi aşar). İçeride tekrar `with_db` ÇAĞRILMAZ.
    pub(crate) fn with_db<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let mut conn = self.db.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut conn)
    }

    /// Sunucu saatine göre düzeltilmiş şimdi (sn).
    pub(crate) fn now(&self) -> i64 {
        util::now(&self.http)
    }

    pub(crate) fn require_session(&self) -> Result<(), CoreError> {
        match self.session() {
            Some(_) => Ok(()),
            None => Err(CoreError::Unauthorized),
        }
    }

    /// Tüm yerel veri: tablolar (kv dahil) + `books/` + `covers/`.
    pub(crate) fn wipe_local(&self) -> Result<(), CoreError> {
        self.with_db(|c| {
            db::wipe_all(c);
            Ok(())
        })?;
        for d in ["books", "covers"] {
            let p = self.root.join(d);
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p)?;
        }
        Ok(())
    }
}

fn read_session(c: &Connection) -> Option<SessionInfo> {
    db::kv_get(c, "token")?;
    Some(SessionInfo {
        server_url: db::kv_get(c, "server_url")?,
        username: db::kv_get(c, "username")?,
        user_id: db::kv_get(c, "user_id")?.parse().ok()?,
        is_admin: db::kv_get(c, "is_admin")? == "1",
    })
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Core>, CoreError> {
        let root = PathBuf::from(data_dir);
        std::fs::create_dir_all(root.join("books"))?;
        std::fs::create_dir_all(root.join("covers"))?;
        let conn = db::open(&root.join("readcrab.db"))?;
        conn.execute(
            "UPDATE downloads SET state = 'queued' WHERE state = 'running'",
            [],
        )?;
        let http = http::Http::new();
        if let (Some(url), Some(token)) = (db::kv_get(&conn, "server_url"), db::kv_get(&conn, "token")) {
            http.set_session(&url, &token);
        }
        Ok(Arc::new(Core {
            db: Mutex::new(conn),
            http,
            root,
            stop: AtomicBool::new(false),
            cancel: Mutex::new(HashSet::new()),
            dl_running: AtomicBool::new(false),
        }))
    }

    /// kv'den; ağ yok.
    pub fn session(&self) -> Option<SessionInfo> {
        self.with_db(|c| Ok(read_session(c))).ok().flatten()
    }

    pub fn login(
        &self,
        server_url: String,
        username: String,
        password: String,
    ) -> Result<SessionInfo, CoreError> {
        let base = http::normalize_url(&server_url)?;
        let (res, token) = self
            .http
            .login(&base, &username, &password)
            .map_err(into_core)?;
        let (prev_url, prev_uid) = self.with_db(|c| {
            Ok((db::kv_get(c, "server_url"), db::kv_get(c, "user_id")))
        })?;
        let same = prev_url.as_deref() == Some(base.as_str())
            && prev_uid.as_deref() == Some(res.id.to_string().as_str());
        // sunucu veya kullanıcı değiştiyse önceki hesabın verisi silinir
        if !same {
            self.wipe_local()?;
        }
        self.with_db(|c| {
            db::kv_set(c, "server_url", &base);
            db::kv_set(c, "username", &res.username);
            db::kv_set(c, "user_id", &res.id.to_string());
            db::kv_set(c, "is_admin", if res.is_admin { "1" } else { "0" });
            db::kv_set(c, "token", &token);
            Ok(())
        })?;
        self.http.set_session(&base, &token);
        Ok(SessionInfo {
            server_url: base,
            username: res.username,
            user_id: res.id,
            is_admin: res.is_admin,
        })
    }

    /// Sunucuya çıkış dener (hata yutulur), sonra TÜM yerel veri silinir.
    pub fn logout(&self) -> Result<(), CoreError> {
        let _ = self
            .http
            .send_json("POST", "/api/logout", &serde_json::json!({}));
        // çalışan indirme nazikçe dursun; sonraki run_downloads bayrağı sıfırlar
        self.stop.store(true, Ordering::SeqCst);
        self.http.clear_session();
        self.wipe_local()
    }
}
