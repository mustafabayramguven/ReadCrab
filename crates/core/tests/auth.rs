mod common;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use common::*;
use readcrab_core::CoreError;
use serde_json::json;

fn hdr<'a>(r: &'a Req, name: &str) -> Option<&'a str> {
    r.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn no_content() -> Reply {
    Reply {
        status: 204,
        headers: vec![],
        body: vec![],
    }
}

/// Giriş + tek seri/tek kitap kütüphanesi. `unauthorized` açılınca kütüphane uçları 401 döner.
fn handler(unauthorized: Arc<AtomicBool>) -> impl Fn(&Req) -> Reply + Send + Sync + 'static {
    move |req: &Req| {
        if is_login(req) {
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
            return if body["password"] == "password1234" {
                login_ok()
            } else {
                login_fail()
            };
        }
        if req.method == "POST" && req.path == "/api/logout" {
            return no_content();
        }
        if unauthorized.load(Ordering::SeqCst) || !has_cookie(req) {
            return login_fail();
        }
        let p = req.path.as_str();
        if p.starts_with("/api/series?") {
            json(
                200,
                &json!({"total": 1, "items": [{"id": 1, "name": "Alfa", "book_count": 1,
                    "read_count": 0, "reading_count": 0, "percent": 0.0,
                    "cover_book_id": null, "cover_v": null, "last_read_at": null}]}),
            )
        } else if p == "/api/series/1" {
            json(
                200,
                &json!({"id": 1, "name": "Alfa", "books": [{"id": 11, "title": "Alfa 1",
                    "format": "cbz", "page_count": 3, "size": 100, "error": null,
                    "status": "unread", "percent": 0.0, "cover_v": 1}]}),
            )
        } else {
            json(404, &json!({"error": "not_found", "message": "Bulunamadı."}))
        }
    }
}

fn start(unauthorized: &Arc<AtomicBool>) -> Mock {
    Mock::start(handler(unauthorized.clone()))
}

#[test]
fn login_sends_json_without_origin_and_session_cookie_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);

    let s = core
        .login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    assert_eq!(s.server_url, mock.base);
    assert_eq!(s.username, "admin");
    assert_eq!(s.user_id, 1);
    assert!(s.is_admin);
    assert_eq!(core.session().unwrap().username, "admin");

    let reqs = mock.requests();
    let login = reqs.iter().find(|r| is_login(r)).unwrap();
    assert_eq!(hdr(login, "content-type"), Some("application/json"));
    assert!(hdr(login, "origin").is_none());
    assert!(hdr(login, "cookie").is_none());

    // oturumlu ilk istek çerezi taşır
    core.sync_now().unwrap();
    let reqs = mock.requests();
    let list = reqs
        .iter()
        .find(|r| r.path.starts_with("/api/series?"))
        .unwrap();
    assert!(has_cookie(list));
    assert!(hdr(list, "origin").is_none());
    assert_eq!(core.list_series(String::new(), false).unwrap().len(), 1);
    mock.stop();
}

#[test]
fn login_url_is_normalized() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);
    // şemasız adres + sondaki eğik çizgi
    let bare = mock.base.trim_start_matches("http://").to_string();
    let s = core
        .login(format!("  {bare}/ "), "admin".into(), "password1234".into())
        .unwrap();
    assert_eq!(s.server_url, mock.base);
    assert!(matches!(
        core.login("   ".into(), "a".into(), "b".into()),
        Err(CoreError::InvalidInput(_))
    ));
    mock.stop();
}

#[test]
fn wrong_password_is_unauthorized() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);
    let r = core.login(mock.base.clone(), "admin".into(), "yanlis".into());
    assert!(matches!(r, Err(CoreError::Unauthorized)));
    assert!(core.session().is_none());
    mock.stop();
}

#[test]
fn logout_posts_empty_json_and_wipes_everything() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    core.sync_now().unwrap();
    assert_eq!(core.list_series(String::new(), false).unwrap().len(), 1);

    core.logout().unwrap();
    let reqs = mock.requests();
    let out = reqs
        .iter()
        .find(|r| r.path == "/api/logout")
        .expect("logout isteği");
    assert_eq!(out.method, "POST");
    assert_eq!(out.body, b"{}");
    assert_eq!(hdr(out, "content-type"), Some("application/json"));
    assert!(has_cookie(out));

    assert!(core.session().is_none());
    assert!(core.list_series(String::new(), false).unwrap().is_empty());
    assert!(matches!(core.sync_now(), Err(CoreError::Unauthorized)));
    mock.stop();
}

#[test]
fn logout_wipes_even_when_server_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    mock.stop();
    core.logout().unwrap();
    assert!(core.session().is_none());
}

#[test]
fn login_to_other_server_wipes_local_data_but_same_server_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let a = start(&flag);
    core.login(a.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    core.sync_now().unwrap();
    assert_eq!(core.list_series(String::new(), false).unwrap().len(), 1);

    // aynı sunucu + aynı kullanıcı: veri korunur
    core.login(a.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    assert_eq!(core.list_series(String::new(), false).unwrap().len(), 1);

    // başka sunucu (başka port): önceki hesabın verisi silinir
    let b = start(&flag);
    assert_ne!(a.base, b.base);
    core.login(b.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    assert!(core.list_series(String::new(), false).unwrap().is_empty());
    assert_eq!(core.session().unwrap().server_url, b.base);
    a.stop();
    b.stop();
}

#[test]
fn unauthorized_sync_keeps_local_data() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let flag = Arc::new(AtomicBool::new(false));
    let mock = start(&flag);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    core.sync_now().unwrap();
    core.save_progress(11, "1".into(), 0.4, "2026-11-08".into())
        .unwrap();

    flag.store(true, Ordering::SeqCst);
    assert!(matches!(core.sync_now(), Err(CoreError::Unauthorized)));
    assert!(matches!(core.sync_push(), Err(CoreError::Unauthorized)));

    // yerel veri ve bekleyen kuyruk durur
    assert_eq!(core.list_series(String::new(), false).unwrap().len(), 1);
    assert_eq!(core.sync_status().pending_ops, 1);
    assert!(core.session().is_some());
    mock.stop();
}
