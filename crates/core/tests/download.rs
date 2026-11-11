mod common;

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use common::*;
use readcrab_core::{Core, CoreError, DownloadState};
use serde_json::json;
use sha2::{Digest, Sha256};

fn close(mut r: Reply) -> Reply {
    r.headers.push(("Connection".into(), "close".into()));
    r
}

fn count(mock: &Mock, f: impl Fn(&Req) -> bool) -> usize {
    mock.requests().iter().filter(|r| f(r)).count()
}

fn page_bytes(n: usize) -> Vec<u8> {
    vec![(n as u8) + 1; 10 + n]
}

fn page_index(req: &Req) -> Option<(i64, usize)> {
    let rest = req.path.strip_prefix("/api/books/")?;
    let (book, tail) = rest.split_once("/pages/")?;
    Some((book.parse().ok()?, tail.parse().ok()?))
}

/// Giriş + tek seri: kitap 11 (cbz, `p11` sayfa), 12 (cbz, 2 sayfa), 7 (epub, 1 bölüm).
fn library(req: &Req, p11: i64) -> Option<Reply> {
    let p = req.path.as_str();
    if is_login(req) {
        return Some(close(login_ok()));
    }
    if !has_cookie(req) {
        return Some(close(login_fail()));
    }
    Some(close(if req.method == "GET" && p.starts_with("/api/series?") {
        json(
            200,
            &json!({"total": 1, "items": [{"id": 1, "name": "Alfa", "book_count": 3,
                "read_count": 0, "reading_count": 0, "percent": 0.0,
                "cover_book_id": 11, "cover_v": 1, "last_read_at": null}]}),
        )
    } else if p == "/api/series/1" {
        json(
            200,
            &json!({"id": 1, "name": "Alfa", "books": [
                {"id": 11, "title": "Alfa 1", "format": "cbz", "page_count": p11, "size": 100,
                 "error": null, "status": "unread", "percent": 0.0, "cover_v": 1},
                {"id": 12, "title": "Alfa 2", "format": "cbz", "page_count": 2, "size": 50,
                 "error": null, "status": "unread", "percent": 0.0, "cover_v": 1},
                {"id": 7, "title": "Metin", "format": "epub", "page_count": 1, "size": 70,
                 "error": null, "status": "unread", "percent": 0.0, "cover_v": 1}]}),
        )
    } else {
        return None;
    }))
}

fn not_found() -> Reply {
    close(json(
        404,
        &json!({"error": "not_found", "message": "Bulunamadı."}),
    ))
}

fn synced(mock: &Mock) -> (tempfile::TempDir, Arc<Core>) {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    core.sync_now().unwrap();
    (dir, core)
}

fn info(core: &Core, id: i64) -> readcrab_core::DownloadInfo {
    core.download_infos()
        .unwrap()
        .into_iter()
        .find(|i| i.book_id == id)
        .expect("indirme satırı")
}

fn offline(mock: Mock) {
    mock.stop();
    // dinleyicinin gerçekten kapanması için kısa bekleme
    std::thread::sleep(Duration::from_millis(300));
}

fn comic_handler(req: &Req) -> Reply {
    if let Some(r) = library(req, 3) {
        return r;
    }
    if let Some((_, n)) = page_index(req) {
        return close(bytes(200, "image/png", page_bytes(n)));
    }
    if req.path == "/api/books/11/cover" {
        return close(bytes(200, "image/jpeg", vec![1; 20]));
    }
    not_found()
}

#[test]
fn comic_download_completes_and_pages_open_offline() {
    let mock = Mock::start(comic_handler);
    let (dir, core) = synced(&mock);

    core.download_enqueue(vec![11]).unwrap();
    assert_eq!(info(&core, 11).state, DownloadState::Queued);
    let run = core.run_downloads().unwrap();
    assert_eq!((run.finished, run.failed), (1, 0));

    let i = info(&core, 11);
    assert_eq!(i.state, DownloadState::Done);
    assert_eq!((i.parts_done, i.parts_total), (3, 3));
    assert_eq!(i.bytes, 10 + 11 + 12);
    assert_eq!(
        core.get_series(1).unwrap().books[0].download.state,
        DownloadState::Done
    );
    // tamamlanmış kitap yeniden kuyruğa alınmaz
    core.download_enqueue(vec![11]).unwrap();
    assert_eq!(info(&core, 11).state, DownloadState::Done);

    offline(mock);
    let p = core.page_path(11, 1).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), page_bytes(1));
    assert!(p.starts_with(dir.path().to_str().unwrap()));
    // kapak indirme sırasında alındı
    assert!(Path::new(&core.cover_path(11, 1).unwrap()).exists());
}

#[test]
fn failed_page_marks_book_failed_and_retry_fetches_only_missing_page() {
    let fixed = Arc::new(AtomicBool::new(false));
    let f = fixed.clone();
    let mock = Mock::start(move |req: &Req| {
        if let Some((11, 1)) = page_index(req) {
            if !f.load(Ordering::SeqCst) {
                return not_found();
            }
        }
        comic_handler(req)
    });
    let (_d, core) = synced(&mock);

    core.download_enqueue(vec![11]).unwrap();
    let run = core.run_downloads().unwrap();
    assert_eq!((run.finished, run.failed), (0, 1));
    let i = info(&core, 11);
    assert_eq!(i.state, DownloadState::Failed);
    assert!(i.error.unwrap().contains("Sayfa 2"));
    assert_eq!(i.parts_done, 2);

    fixed.store(true, Ordering::SeqCst);
    core.download_enqueue(vec![11]).unwrap();
    let run = core.run_downloads().unwrap();
    assert_eq!((run.finished, run.failed), (1, 0));
    assert_eq!(info(&core, 11).state, DownloadState::Done);

    let page = |n: usize| count(&mock, |r| page_index(r) == Some((11, n)));
    assert_eq!(page(0), 1, "tamamlanmış sayfa yeniden istenmemeli");
    assert_eq!(page(2), 1);
    assert_eq!(page(1), 2, "başarısız sayfa yalnız bir kez yeniden istenir (4xx yeniden denenmez)");
    mock.stop();
}

#[test]
fn text_download_rewrites_chapter_images() {
    const DOC: &str = "<!doctype html>\n<html lang=\"tr\">\n<head>\n<meta charset=\"utf-8\">\n\
        <title>T</title>\n</head>\n<body>\n<p>Merhaba</p>\
        <img src=\"/api/books/7/asset?p=OEBPS%2Fimg%2Fa%2Epng\">\
        <img src=\"/api/books/7/asset?p=gone\">\n</body>\n</html>";
    let mock = Mock::start(|req: &Req| {
        if let Some(r) = library(req, 3) {
            return r;
        }
        match req.path.as_str() {
            "/api/books/7/toc" => close(json(
                200,
                &json!({"chapters": [{"index": 0, "title": "Giriş"}]}),
            )),
            "/api/books/7/chapters/0" => {
                close(bytes(200, "text/html; charset=utf-8", DOC.as_bytes().to_vec()))
            }
            "/api/books/7/asset?p=OEBPS%2Fimg%2Fa%2Epng" => {
                close(bytes(200, "image/png", vec![9; 5]))
            }
            _ => not_found(),
        }
    });
    let (dir, core) = synced(&mock);

    core.download_enqueue(vec![7]).unwrap();
    let run = core.run_downloads().unwrap();
    assert_eq!((run.finished, run.failed), (1, 0));

    let hash = hex::encode(Sha256::digest(b"OEBPS%2Fimg%2Fa%2Epng"));
    let name = format!("{}.png", &hash[..16]);
    let html = std::fs::read_to_string(dir.path().join("books/7/c0.html")).unwrap();
    assert!(html.contains("<p>Merhaba</p>"));
    assert!(html.contains(&format!("src=\"/book/7/a/{name}\"")));
    assert!(html.contains("src=\"\""), "404 görselin src'si boşaltılmalı");
    assert!(!html.contains("/api/books/"));
    assert_eq!(
        std::fs::read(dir.path().join("books/7/a").join(&name)).unwrap(),
        vec![9; 5]
    );

    // içindekiler ve bölüm çevrimdışı da okunur
    offline(mock);
    let t = core.text_info(7).unwrap();
    assert_eq!(t.chapter_count, 1);
    assert_eq!(t.toc[0].title, "Giriş");
    core.ensure_chapter(7, 0).unwrap();
    assert!(matches!(core.ensure_chapter(7, 1), Err(CoreError::Offline)));
}

#[test]
fn cover_is_cached_per_version() {
    let mock = Mock::start(comic_handler);
    let (dir, core) = synced(&mock);
    let covers = |m: &Mock| count(m, |r| r.path == "/api/books/11/cover");

    let p1 = core.cover_path(11, 1).unwrap();
    assert!(p1.ends_with("covers/11-1.jpg"));
    assert_eq!(std::fs::read(&p1).unwrap(), vec![1; 20]);
    assert_eq!(covers(&mock), 1);
    assert_eq!(core.cover_path(11, 1).unwrap(), p1);
    assert_eq!(covers(&mock), 1, "ikinci çağrı ağa gitmemeli");

    // sürüm değişince yenisi alınır, eskisi silinir
    let p2 = core.cover_path(11, 2).unwrap();
    assert!(p2.ends_with("covers/11-2.jpg"));
    assert_eq!(covers(&mock), 2);
    assert!(!dir.path().join("covers/11-1.jpg").exists());
    assert!(dir.path().join("covers/11-2.jpg").exists());

    // çevrimdışı: olan sürüm açılır, olmayan Offline
    offline(mock);
    assert_eq!(core.cover_path(11, 2).unwrap(), p2);
    assert!(matches!(core.cover_path(11, 3), Err(CoreError::Offline)));
}

#[test]
fn reader_calls_map_missing_pages_and_offline() {
    // 12 numaralı kitabın yalnız 0 ve 1 numaralı sayfaları var
    let mock = Mock::start(|req: &Req| {
        if let Some((_, n)) = page_index(req) {
            if n >= 2 {
                return not_found();
            }
        }
        comic_handler(req)
    });
    let (_d, core) = synced(&mock);
    assert!(matches!(core.page_path(12, 99), Err(CoreError::NotFound)));
    assert!(matches!(core.page_path(12, -1), Err(CoreError::InvalidInput(_))));
    core.page_path(12, 0).unwrap();
    offline(mock);
    core.page_path(12, 0).unwrap();
    assert!(matches!(core.page_path(12, 1), Err(CoreError::Offline)));
    assert!(matches!(
        core.search_text(7, "a".into()),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        core.search_text(7, "arama".into()),
        Err(CoreError::Network(_))
    ));
}

#[test]
fn clear_cache_keeps_downloaded_books_and_usage_is_consistent() {
    let mock = Mock::start(comic_handler);
    let (_d, core) = synced(&mock);
    core.download_enqueue(vec![11]).unwrap();
    core.run_downloads().unwrap();
    // 12 indirilmedi: okunan sayfa yalnız önbellekte
    core.page_path(12, 0).unwrap();

    let u = core.storage_usage().unwrap();
    assert_eq!(u.downloads_bytes, 10 + 11 + 12);
    assert_eq!(u.cache_bytes, 10);
    assert_eq!(u.covers_bytes, 20);
    assert!(u.db_bytes >= 0);

    assert_eq!(core.clear_cache().unwrap(), 10);
    let u = core.storage_usage().unwrap();
    assert_eq!((u.downloads_bytes, u.cache_bytes), (33, 0));
    assert_eq!(info(&core, 11).state, DownloadState::Done);

    offline(mock);
    core.page_path(11, 0).unwrap();
    assert!(matches!(core.page_path(12, 0), Err(CoreError::Offline)));

    // indirmeyi kaldırmak dosyaları da siler
    core.download_remove(vec![11]).unwrap();
    assert!(core.download_infos().unwrap().is_empty());
    assert!(matches!(core.page_path(11, 0), Err(CoreError::Offline)));
    assert_eq!(core.storage_usage().unwrap().downloads_bytes, 0);
}

#[test]
fn download_stop_halts_run_and_requeues() {
    let mock = Mock::start(|req: &Req| {
        if let Some(r) = library(req, 10) {
            return r;
        }
        if page_index(req).is_some() {
            std::thread::sleep(Duration::from_millis(200));
            return close(bytes(200, "image/png", vec![1; 4]));
        }
        not_found()
    });
    let (_d, core) = synced(&mock);
    core.download_enqueue(vec![11]).unwrap();

    let c2 = core.clone();
    let h = std::thread::spawn(move || c2.run_downloads());
    let t0 = Instant::now();
    while count(&mock, |r| page_index(r).is_some()) == 0 {
        assert!(t0.elapsed() < Duration::from_secs(10), "indirme başlamadı");
        std::thread::sleep(Duration::from_millis(20));
    }
    core.download_stop();
    let run = h.join().unwrap().unwrap();
    assert_eq!((run.finished, run.failed), (0, 0));

    let i = info(&core, 11);
    assert_eq!(i.state, DownloadState::Queued);
    assert!(i.parts_done < 10, "kalan parçalar indirilmemeli");
    mock.stop();
}

#[test]
fn unauthorized_download_requeues_and_errors() {
    let mock = Mock::start(|req: &Req| {
        if let Some(r) = library(req, 3) {
            return r;
        }
        close(login_fail())
    });
    let (_d, core) = synced(&mock);
    core.download_enqueue(vec![11]).unwrap();
    assert!(matches!(core.run_downloads(), Err(CoreError::Unauthorized)));
    assert_eq!(info(&core, 11).state, DownloadState::Queued);
    mock.stop();
}
