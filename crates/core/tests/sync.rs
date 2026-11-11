mod common;

use std::sync::{Arc, Mutex};

use common::*;
use readcrab_core::{Core, CoreError, ReadStatus};
use serde_json::{json, Value};

const DATE: &str = "2026-11-08";

fn close(mut r: Reply) -> Reply {
    r.headers.push(("Connection".into(), "close".into()));
    r
}

fn no_content() -> Reply {
    close(Reply {
        status: 204,
        headers: vec![],
        body: vec![],
    })
}

fn body_of(r: &Req) -> Value {
    serde_json::from_slice(&r.body).unwrap()
}

fn hdr<'a>(r: &'a Req, name: &str) -> Option<&'a str> {
    r.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn count(mock: &Mock, f: impl Fn(&Req) -> bool) -> usize {
    mock.requests().iter().filter(|r| f(r)).count()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Kütüphane: "Alfa" (kitap 11 okunuyor %50, kitap 12 okunmamış) ve "Beta" (kitap 21 okunmamış).
/// Giriş, seri uçları, ilerleme/yer imi GET'leri ve ilerleme/durum/ping yazmaları; çerezsiz istek 401 alır.
fn base_reply(req: &Req) -> Reply {
    let p = req.path.as_str();
    if is_login(req) {
        return close(login_ok());
    }
    if !has_cookie(req) {
        return close(login_fail());
    }
    close(if req.method == "GET" && p.starts_with("/api/series?") {
        json(
            200,
            &json!({"total": 2, "items": [
                {"id": 1, "name": "Alfa", "book_count": 2, "read_count": 0, "reading_count": 1,
                 "percent": 0.25, "cover_book_id": 11, "cover_v": 1, "last_read_at": 1000},
                {"id": 2, "name": "Beta", "book_count": 1, "read_count": 0, "reading_count": 0,
                 "percent": 0.0, "cover_book_id": null, "cover_v": null, "last_read_at": null}]}),
        )
    } else if p == "/api/series/1" {
        json(
            200,
            &json!({"id": 1, "name": "Alfa", "books": [
                {"id": 11, "title": "Alfa 1", "format": "cbz", "page_count": 3, "size": 100,
                 "error": null, "status": "reading", "percent": 0.5, "cover_v": 1},
                {"id": 12, "title": "Alfa 2", "format": "cbz", "page_count": 2, "size": 50,
                 "error": null, "status": "unread", "percent": 0.0, "cover_v": 1}]}),
        )
    } else if p == "/api/series/2" {
        json(
            200,
            &json!({"id": 2, "name": "Beta", "books": [
                {"id": 21, "title": "Beta 1", "format": "epub", "page_count": 2, "size": 70,
                 "error": null, "status": "unread", "percent": 0.0, "cover_v": 1}]}),
        )
    } else if req.method == "GET" && p == "/api/books/11/progress" {
        json(
            200,
            &json!({"locator": "1", "percent": 0.5, "status": "reading", "updated_at": 1000}),
        )
    } else if req.method == "GET" && p.ends_with("/bookmarks") {
        json(200, &json!({"items": []}))
    } else if req.method == "PUT" && p.ends_with("/progress") {
        json(
            200,
            &json!({"status": "reading", "percent": 0.5, "applied": true, "updated_at": 1}),
        )
    } else if req.method == "PUT" && p.ends_with("/status") {
        json(200, &json!({"status": "reading", "applied": true, "updated_at": 1}))
    } else if req.method == "POST" && p == "/api/reading/ping" {
        no_content()
    } else {
        json(404, &json!({"error": "not_found", "message": "Bulunamadı."}))
    })
}

fn logged_in(mock: &Mock) -> (tempfile::TempDir, Arc<Core>) {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    (dir, core)
}

#[test]
fn first_sync_builds_library_and_fetches_progress_only_for_changed_books() {
    let mock = Mock::start(base_reply);
    let (_d, core) = logged_in(&mock);

    let rep = core.sync_now().unwrap();
    assert_eq!(rep.pulled_books, 1);

    let series = core.list_series(String::new(), false).unwrap();
    assert_eq!(
        series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["Alfa", "Beta"]
    );
    assert_eq!(series[0].book_count, 2);
    assert_eq!(series[0].reading_count, 1);
    assert!((series[0].percent - 0.25).abs() < 1e-9);
    assert_eq!(series[0].last_read_at, Some(1000));
    assert_eq!(series[0].cover_book_id, Some(11));
    assert_eq!(series[1].cover_book_id, None);

    let detail = core.get_series(1).unwrap();
    assert_eq!(detail.books.len(), 2);
    assert!(detail.books[0].is_comic);
    assert_eq!(detail.books[0].status, ReadStatus::Reading);
    assert!(!core.get_series(2).unwrap().books[0].is_comic);

    let cont = core.continue_reading().unwrap();
    assert_eq!(cont.len(), 1);
    assert_eq!(cont[0].book_id, 11);
    assert_eq!(cont[0].series_name, "Alfa");
    assert!((cont[0].percent - 0.5).abs() < 1e-9);
    assert_eq!(core.get_progress(11).unwrap().locator, "1");

    // arama ad ve kitap başlığına göre
    assert_eq!(core.list_series("alfa".into(), false).unwrap().len(), 1);
    assert_eq!(core.list_series("BETA 1".into(), false).unwrap()[0].name, "Beta");
    assert!(core.list_series("yok".into(), false).unwrap().is_empty());
    assert!(core.list_series(String::new(), true).unwrap().is_empty());

    let book = core.get_book(12).unwrap();
    assert_eq!(book.prev_book_id, Some(11));
    assert_eq!(book.next_book_id, None);

    // ilerleme yalnız okunmamış/0 olmayan kitap için çekildi
    let progress_gets = count(&mock, |r| {
        r.method == "GET" && r.path.starts_with("/api/books/") && r.path.ends_with("/progress")
    });
    assert_eq!(progress_gets, 1);
    assert_eq!(count(&mock, |r| r.path.starts_with("/api/series/")), 2);
    mock.stop();
}

#[test]
fn second_sync_with_same_signatures_skips_detail_and_progress() {
    let mock = Mock::start(base_reply);
    let (_d, core) = logged_in(&mock);
    core.sync_now().unwrap();
    let details = count(&mock, |r| r.path.starts_with("/api/series/"));
    let progress = count(&mock, |r| r.path.ends_with("/progress"));

    let rep = core.sync_now().unwrap();
    assert_eq!(rep.pulled_books, 0);
    assert_eq!(count(&mock, |r| r.path.starts_with("/api/series/")), details);
    assert_eq!(count(&mock, |r| r.path.ends_with("/progress")), progress);
    // liste yine istenir
    assert_eq!(count(&mock, |r| r.path.starts_with("/api/series?")), 2);
    assert!(core.sync_status().last_ok_at.is_some());
    assert_eq!(core.sync_status().last_error, None);
    mock.stop();
}

#[test]
fn offline_queue_survives_and_is_pushed_when_server_returns() {
    let mock = Mock::start(base_reply);
    let (_d, core) = logged_in(&mock);
    let port = mock.port();
    mock.stop();
    // dinleyicinin gerçekten kapanması için kısa bekleme (çevrimdışı simülasyonu)
    std::thread::sleep(std::time::Duration::from_millis(300));

    core.save_progress(11, "2".into(), 0.5, DATE.into()).unwrap();
    core.set_status(11, ReadStatus::Reading, DATE.into()).unwrap();
    core.add_bookmark(11, "2".into(), "Sayfa 3".into()).unwrap();
    let id = core.ping_start(11, 0.0, DATE.into()).unwrap();
    core.ping_update(id.clone(), 42, 3, 0.5).unwrap();
    core.ping_close(id).unwrap();
    // ilerleme + durum + yer imi + ping
    assert_eq!(core.sync_status().pending_ops, 4);

    assert!(matches!(core.sync_now(), Err(CoreError::Network(_))));
    assert_eq!(core.sync_status().pending_ops, 4);
    assert!(core.sync_status().last_error.is_some());

    let posted = Arc::new(Mutex::new(Vec::<String>::new()));
    let p = posted.clone();
    let mock = Mock::start_on(port, move |req: &Req| {
        if req.method == "POST" && req.path == "/api/books/11/bookmarks" {
            p.lock().unwrap().push("bookmark".into());
            return close(json(
                201,
                &json!({"id": 5, "locator": "2", "label": "Sayfa 3", "created_at": 1}),
            ));
        }
        base_reply(req)
    });
    core.sync_now().unwrap();
    assert_eq!(core.sync_status().pending_ops, 0);
    assert_eq!(core.sync_status().last_error, None);
    assert_eq!(posted.lock().unwrap().len(), 1);

    let reqs = mock.requests();
    let put = reqs
        .iter()
        .find(|r| r.method == "PUT" && r.path == "/api/books/11/progress")
        .expect("ilerleme PUT");
    let body = body_of(put);
    assert_eq!(body["locator"], "2");
    assert_eq!(body["local_date"], DATE);
    assert!((body["client_ts"].as_i64().unwrap() - now_secs()).abs() < 10);
    assert_eq!(hdr(put, "content-type"), Some("application/json"));
    assert!(has_cookie(put));
    mock.stop();
}

#[test]
fn stale_put_response_pulls_server_progress() {
    let mock = Mock::start(|req: &Req| {
        if req.method == "PUT" && req.path == "/api/books/11/progress" {
            return close(json(
                200,
                &json!({"status": "reading", "percent": 0.9, "applied": false,
                        "updated_at": 4_000_000_000i64}),
            ));
        }
        if req.method == "GET" && req.path == "/api/books/11/progress" {
            return close(json(
                200,
                &json!({"locator": "9", "percent": 0.9, "status": "reading",
                        "updated_at": 4_000_000_000i64}),
            ));
        }
        base_reply(req)
    });
    let (_d, core) = logged_in(&mock);
    core.save_progress(11, "5".into(), 0.5, DATE.into()).unwrap();
    core.sync_push().unwrap();

    assert!(
        count(&mock, |r| r.method == "GET" && r.path == "/api/books/11/progress") >= 1,
        "applied=false sonrası sunucu ilerlemesi çekilmeli"
    );
    let p = core.get_progress(11).unwrap();
    assert_eq!(p.locator, "9");
    assert!((p.percent - 0.9).abs() < 1e-9);
    assert_eq!(core.sync_status().pending_ops, 0);
    mock.stop();
}

#[test]
fn newer_server_progress_cancels_pending_local_op() {
    let mock = Mock::start(|req: &Req| {
        if req.method == "GET" && req.path == "/api/books/11/progress" {
            return close(json(
                200,
                &json!({"locator": "8", "percent": 0.8, "status": "reading",
                        "updated_at": 4_000_000_000i64}),
            ));
        }
        base_reply(req)
    });
    let (_d, core) = logged_in(&mock);
    core.save_progress(11, "7".into(), 0.7, DATE.into()).unwrap();
    assert_eq!(core.sync_status().pending_ops, 1);

    core.pull_book(11).unwrap();
    assert_eq!(core.get_progress(11).unwrap().locator, "8");
    assert_eq!(core.sync_status().pending_ops, 0);
    // itilecek bir şey kalmadı
    assert_eq!(core.sync_push().unwrap(), 0);
    assert_eq!(count(&mock, |r| r.method == "PUT"), 0);
    mock.stop();
}

#[test]
fn newer_local_progress_is_kept_and_pushed() {
    let mock = Mock::start(|req: &Req| {
        if req.method == "GET" && req.path == "/api/books/11/progress" {
            return close(json(
                200,
                &json!({"locator": "1", "percent": 0.1, "status": "reading", "updated_at": 1000}),
            ));
        }
        base_reply(req)
    });
    let (_d, core) = logged_in(&mock);
    core.save_progress(11, "7".into(), 0.7, DATE.into()).unwrap();

    core.pull_book(11).unwrap();
    assert_eq!(core.get_progress(11).unwrap().locator, "7");
    assert_eq!(core.sync_status().pending_ops, 1);

    assert_eq!(core.sync_push().unwrap(), 1);
    let reqs = mock.requests();
    let put = reqs
        .iter()
        .find(|r| r.method == "PUT" && r.path == "/api/books/11/progress")
        .unwrap();
    assert_eq!(body_of(put)["locator"], "7");
    mock.stop();
}

#[test]
fn bookmarks_roundtrip_with_server() {
    let server: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let s = server.clone();
    let mock = Mock::start(move |req: &Req| {
        let mut list = s.lock().unwrap();
        if req.method == "POST" && req.path == "/api/books/11/bookmarks" {
            let b = body_of(req);
            let id = 70 + list.len() as i64;
            let item = json!({"id": id, "locator": b["locator"], "label": b["label"],
                              "created_at": 5});
            list.push(item.clone());
            return close(json(201, &item));
        }
        if req.method == "DELETE" && req.path.starts_with("/api/bookmarks/") {
            let id: i64 = req.path.rsplit('/').next().unwrap().parse().unwrap();
            list.retain(|b| b["id"] != id);
            return no_content();
        }
        if req.method == "GET" && req.path == "/api/books/11/bookmarks" {
            return close(json(200, &json!({ "items": *list })));
        }
        drop(list);
        base_reply(req)
    });
    let (_d, core) = logged_in(&mock);

    // çevrimdışı eklenir: bekleyen
    let b = core.add_bookmark(11, "2".into(), "Sayfa 3".into()).unwrap();
    assert!(b.pending);
    core.sync_push().unwrap();
    let items = core.list_bookmarks(11).unwrap();
    assert_eq!(items.len(), 1);
    assert!(!items[0].pending);
    assert_eq!(server.lock().unwrap().len(), 1);

    // silme: DELETE gider (Content-Type json)
    core.delete_bookmark(b.local_id).unwrap();
    assert!(core.list_bookmarks(11).unwrap().is_empty());
    assert_eq!(core.sync_status().pending_ops, 1);
    core.sync_push().unwrap();
    let reqs = mock.requests();
    let del = reqs.iter().find(|r| r.method == "DELETE").expect("DELETE");
    assert_eq!(del.path, "/api/bookmarks/70");
    assert_eq!(hdr(del, "content-type"), Some("application/json"));
    assert!(server.lock().unwrap().is_empty());
    assert_eq!(core.sync_status().pending_ops, 0);

    // sunucuda sonradan eklenen yer imi yerele gelir; sunucuda silinen yerelden de silinir
    server
        .lock()
        .unwrap()
        .push(json!({"id": 90, "locator": "4", "label": "Web", "created_at": 9}));
    core.pull_book(11).unwrap();
    let items = core.list_bookmarks(11).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Web");
    assert!(!items[0].pending);
    server.lock().unwrap().clear();
    core.pull_book(11).unwrap();
    assert!(core.list_bookmarks(11).unwrap().is_empty());
    mock.stop();
}

#[test]
fn ping_is_idempotent_and_removed_after_close() {
    let mock = Mock::start(base_reply);
    let (_d, core) = logged_in(&mock);
    let id = core.ping_start(11, 0.0, DATE.into()).unwrap();
    assert_eq!(id.len(), 32);
    core.ping_update(id.clone(), 10, 1, 0.1).unwrap();
    core.ping_update(id.clone(), 25, 2, 0.2).unwrap();
    assert_eq!(core.sync_status().pending_ops, 1);

    assert_eq!(core.sync_push().unwrap(), 1);
    let pings = |mock: &Mock| -> Vec<Req> {
        mock.requests()
            .into_iter()
            .filter(|r| r.path == "/api/reading/ping")
            .collect()
    };
    let sent = pings(&mock);
    assert_eq!(sent.len(), 1);
    let body = body_of(&sent[0]);
    assert_eq!(body["client_id"], id.as_str());
    assert_eq!(body["active_seconds"], 25);
    assert_eq!(body["pages_read"], 2);
    assert_eq!(body["book_id"], 11);
    assert_eq!(body["local_date"], DATE);
    assert_eq!(core.sync_status().pending_ops, 0);

    // yeni süre biriktikçe aynı client_id ile yeniden gider
    core.ping_update(id.clone(), 40, 3, 0.3).unwrap();
    assert_eq!(core.sync_push().unwrap(), 1);
    assert_eq!(body_of(&pings(&mock)[1])["active_seconds"], 40);

    // kapat + gönderildi: satır silinir
    core.ping_close(id.clone()).unwrap();
    core.sync_push().unwrap();
    assert_eq!(pings(&mock).len(), 2);
    assert!(matches!(
        core.ping_update(id, 50, 4, 0.4),
        Err(CoreError::NotFound)
    ));
    mock.stop();
}

#[test]
fn push_order_follows_last_op() {
    let mock = Mock::start(base_reply);
    let (_d, core) = logged_in(&mock);
    // kitap 11: önce ilerleme, sonra durum (last_op='s') -> ilerleme önce gider
    core.save_progress(11, "1".into(), 0.5, DATE.into()).unwrap();
    core.set_status(11, ReadStatus::Reading, DATE.into()).unwrap();
    // kitap 12: önce durum, sonra ilerleme (last_op='p') -> durum önce gider
    core.set_status(12, ReadStatus::Reading, DATE.into()).unwrap();
    core.save_progress(12, "1".into(), 0.5, DATE.into()).unwrap();
    assert_eq!(core.sync_push().unwrap(), 4);

    let order = |book: i64| -> Vec<String> {
        mock.requests()
            .into_iter()
            .filter(|r| r.method == "PUT" && r.path.starts_with(&format!("/api/books/{book}/")))
            .map(|r| r.path.rsplit('/').next().unwrap().to_string())
            .collect()
    };
    assert_eq!(order(11), ["progress", "status"]);
    assert_eq!(order(12), ["status", "progress"]);

    let put = mock
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/books/12/status")
        .unwrap();
    let body = body_of(&put);
    assert_eq!(body["status"], "reading");
    assert!(body["client_ts"].as_i64().unwrap() > 0);
    mock.stop();
}

#[test]
fn permanent_put_errors_drop_the_op_but_server_errors_keep_it() {
    let fail = Arc::new(Mutex::new(404u16));
    let f = fail.clone();
    let mock = Mock::start(move |req: &Req| {
        if req.method == "PUT" && req.path == "/api/books/11/progress" {
            let code = *f.lock().unwrap();
            return close(json(code, &json!({"error": "x", "message": "hata"})));
        }
        base_reply(req)
    });
    let (_d, core) = logged_in(&mock);
    core.save_progress(11, "1".into(), 0.5, DATE.into()).unwrap();

    // 404: kalıcı hata, işlem düşer
    assert_eq!(core.sync_push().unwrap(), 0);
    assert_eq!(core.sync_status().pending_ops, 0);

    // 500: kuyruk korunur
    core.save_progress(11, "2".into(), 0.5, DATE.into()).unwrap();
    *fail.lock().unwrap() = 500;
    assert!(matches!(core.sync_push(), Err(CoreError::Server(_))));
    assert_eq!(core.sync_status().pending_ops, 1);
    mock.stop();
}
