mod common;

use std::{ffi::OsStr, os::unix::ffi::OsStrExt, sync::atomic::Ordering};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::{body_json, login_as_admin, make_cbz, make_epub, png, test_app, EpubOpts};
use http_body_util::BodyExt;
use kavita_rs::scan::scan_once;
use tower::ServiceExt;

#[tokio::test]
async fn scan_builds_series_and_books() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let img = png(120, 180, [10, 20, 30]);
    make_cbz(
        dirs.books_dir.path(),
        "root_book.cbz",
        &[("p1.png", img.clone()), ("p2.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "Manga/Berserk/Cilt 10.cbz",
        &[("01.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "Manga/Berserk/Cilt 2.cbz",
        &[("01.png", img.clone()), ("02.png", img.clone())],
    );
    make_epub(
        dirs.books_dir.path(),
        "Roman/dune.epub",
        EpubOpts {
            title: Some("Çöl Gezegeni".to_string()),
            svg_cover: false,
        },
    );

    assert!(scan_once(&dirs.state).await);

    let status_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/scan-status")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_res.status(), StatusCode::OK);
    let st = body_json(status_res).await;
    assert_eq!(st["running"], false);
    assert_eq!(st["total"], 4);
    assert_eq!(st["done"], 4);

    let series_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(series_res.status(), StatusCode::OK);
    let series_json = body_json(series_res).await;
    assert_eq!(series_json["total"], 3);
    let items = series_json["items"].as_array().unwrap();
    let names: Vec<&str> = items.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["Diğer", "Manga / Berserk", "Roman"]);

    let berserk_id = items[1]["id"].as_i64().unwrap();
    let detail_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/series/{berserk_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_res.status(), StatusCode::OK);
    let detail = body_json(detail_res).await;
    let books = detail["books"].as_array().unwrap();
    assert_eq!(books.len(), 2);
    assert_eq!(books[0]["title"], "Cilt 2");
    assert_eq!(books[1]["title"], "Cilt 10");

    let roman_id = items[2]["id"].as_i64().unwrap();
    let roman_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/series/{roman_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let roman_detail = body_json(roman_res).await;
    let epub_id = roman_detail["books"][0]["id"].as_i64().unwrap();

    let book_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{epub_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(book_res.status(), StatusCode::OK);
    let book_json = body_json(book_res).await;
    assert_eq!(book_json["title"], "Çöl Gezegeni");
    assert_eq!(book_json["format"], "epub");
    assert_eq!(book_json["page_count"], 3);
    assert_eq!(book_json["status"], "unread");
}

#[tokio::test]
async fn search_and_sort() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let img = png(32, 32, [100, 100, 100]);
    make_cbz(
        dirs.books_dir.path(),
        "Zeta/one.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "Alpha/özel_başlık.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(dirs.books_dir.path(), "Beta/two.cbz", &[("p1.png", img)]);

    assert!(scan_once(&dirs.state).await);

    dirs.db
        .run(|conn| {
            conn.execute(
                "UPDATE series SET added_at = 100 WHERE name = 'Zeta'",
                [],
            )?;
            conn.execute(
                "UPDATE series SET added_at = 300 WHERE name = 'Beta'",
                [],
            )?;
            conn.execute(
                "UPDATE series SET added_at = 200 WHERE name = 'Alpha'",
                [],
            )?;
            let zeta_book: i64 = conn.query_row(
                "SELECT b.id FROM books b JOIN series s ON s.id = b.series_id WHERE s.name = 'Zeta'",
                [],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (1, ?1, '0', 0.5, 'reading', 9999)",
                [zeta_book],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let name_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?sort=name")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let name_json = body_json(name_res).await;
    let by_name: Vec<&str> = name_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(by_name, vec!["Alpha", "Beta", "Zeta"]);

    let added_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?sort=added")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let added_json = body_json(added_res).await;
    let by_added: Vec<&str> = added_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(by_added, vec!["Beta", "Alpha", "Zeta"]);

    let recent_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?sort=recent")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let recent_json = body_json(recent_res).await;
    assert_eq!(recent_json["items"][0]["name"], "Zeta");

    let q_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?q=%C3%B6zel_ba%C5%9Fl%C4%B1k")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let q_json = body_json(q_res).await;
    assert_eq!(q_json["total"], 1);
    assert_eq!(q_json["items"][0]["name"], "Alpha");
}

#[tokio::test]
async fn series_status_filter() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let img = png(32, 32, [50, 60, 70]);
    make_cbz(
        dirs.books_dir.path(),
        "SeriUnread/b1.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "SeriReading/b1.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "SeriPartialFinished/b1.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "SeriPartialFinished/b2.cbz",
        &[("p1.png", img.clone())],
    );
    make_cbz(
        dirs.books_dir.path(),
        "SeriFinished/b1.cbz",
        &[("p1.png", img)],
    );

    assert!(scan_once(&dirs.state).await);

    dirs.db
        .run(|conn| {
            let reading_book: i64 = conn.query_row(
                "SELECT id FROM books WHERE rel_path = 'SeriReading/b1.cbz'",
                [],
                |r| r.get(0),
            )?;
            let partial_book: i64 = conn.query_row(
                "SELECT id FROM books WHERE rel_path = 'SeriPartialFinished/b1.cbz'",
                [],
                |r| r.get(0),
            )?;
            let finished_book: i64 = conn.query_row(
                "SELECT id FROM books WHERE rel_path = 'SeriFinished/b1.cbz'",
                [],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (1, ?1, '0', 0.4, 'reading', 100)",
                [reading_book],
            )?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (1, ?1, '0', 1.0, 'finished', 100)",
                [partial_book],
            )?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (1, ?1, '0', 1.0, 'finished', 100)",
                [finished_book],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let reading_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?status=reading&sort=name")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let reading_json = body_json(reading_res).await;
    let reading_names: Vec<&str> = reading_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(reading_names, vec!["SeriPartialFinished", "SeriReading"]);
    assert_eq!(reading_json["items"][0]["percent"], 0.5);
    assert_eq!(reading_json["items"][1]["reading_count"], 1);

    let fin_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?status=finished")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let fin_json = body_json(fin_res).await;
    assert_eq!(fin_json["total"], 1);
    assert_eq!(fin_json["items"][0]["name"], "SeriFinished");
    assert_eq!(fin_json["items"][0]["percent"], 1.0);

    let unread_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?status=unread")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let unread_json = body_json(unread_res).await;
    assert_eq!(unread_json["total"], 1);
    assert_eq!(unread_json["items"][0]["name"], "SeriUnread");
    assert_eq!(unread_json["items"][0]["percent"], 0.0);

    let bad_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series?status=bogus")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn scan_non_blocking() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    dirs.state.scan.running.store(true, Ordering::SeqCst);

    let health_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_res.status(), StatusCode::OK);

    let conflict_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/scan")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict_res.status(), StatusCode::CONFLICT);

    dirs.state.scan.running.store(false, Ordering::SeqCst);

    let start_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/scan")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start_res.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn deleted_file_hidden_but_history_kept() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let cbz_path = make_cbz(
        dirs.books_dir.path(),
        "Silinecek/kitap.cbz",
        &[("p1.png", png(60, 90, [10, 20, 30]))],
    );
    assert!(scan_once(&dirs.state).await);

    let book_id: i64 = dirs
        .db
        .run(|conn| {
            let id: i64 = conn.query_row("SELECT id FROM books LIMIT 1", [], |r| r.get(0))?;
            conn.execute(
                "INSERT INTO reading_sessions(
                    user_id, book_id, client_id, started_at, ended_at,
                    active_seconds, pages_read, start_percent, end_percent, local_date
                 ) VALUES (1, ?1, '0123456789abcdef0123456789abcdef', 100, 200, 100, 5, 0.0, 0.5, '2026-03-01')",
                [id],
            )?;
            Ok(id)
        })
        .await
        .unwrap();

    std::fs::remove_file(&cbz_path).unwrap();
    assert!(scan_once(&dirs.state).await);

    let series_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let series_json = body_json(series_res).await;
    assert_eq!(series_json["total"], 0);

    let book_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{book_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(book_res.status(), StatusCode::NOT_FOUND);

    let cover_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{book_id}/cover"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cover_res.status(), StatusCode::OK);

    let (sessions_count, missing_flag): (i64, i64) = dirs
        .db
        .run(move |conn| {
            let row = conn.query_row(
                "SELECT COUNT(*), b.missing
                 FROM reading_sessions rs
                 JOIN books b ON b.id = rs.book_id
                 WHERE rs.book_id = ?1",
                [book_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(row)
        })
        .await
        .unwrap();
    assert_eq!(sessions_count, 1);
    assert_eq!(missing_flag, 1);
}

#[tokio::test]
async fn moved_file_revives_same_book_id() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let old_path = make_cbz(
        dirs.books_dir.path(),
        "EskiKlasor/eski_ad.cbz",
        &[
            ("p1.png", png(40, 60, [200, 100, 50])),
            ("p2.png", png(40, 60, [50, 100, 200])),
        ],
    );
    assert!(scan_once(&dirs.state).await);

    let orig_id: i64 = dirs
        .db
        .run(|conn| {
            let id: i64 = conn.query_row("SELECT id FROM books LIMIT 1", [], |r| r.get(0))?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (1, ?1, '1', 0.5, 'reading', 500)",
                [id],
            )?;
            Ok(id)
        })
        .await
        .unwrap();

    let new_dir = dirs.books_dir.path().join("YeniKlasor");
    std::fs::create_dir_all(&new_dir).unwrap();
    let new_path = new_dir.join("yeni_ad.cbz");
    std::fs::rename(&old_path, &new_path).unwrap();

    assert!(scan_once(&dirs.state).await);

    let book_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{orig_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(book_res.status(), StatusCode::OK);
    let book_json = body_json(book_res).await;
    assert_eq!(book_json["id"], orig_id);
    assert_eq!(book_json["title"], "yeni_ad");
    assert_eq!(book_json["series_name"], "YeniKlasor");
    assert_eq!(book_json["status"], "reading");
    assert_eq!(book_json["percent"], 0.5);
    assert_eq!(book_json["locator"], "1");
}

#[tokio::test]
async fn symlink_ignored_by_scan() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let outside = tempfile::tempdir().unwrap();
    let real_cbz = make_cbz(
        outside.path(),
        "disari.cbz",
        &[("p1.png", png(20, 20, [1, 2, 3]))],
    );
    let link_path = dirs.books_dir.path().join("link.cbz");
    std::os::unix::fs::symlink(&real_cbz, &link_path).unwrap();

    assert!(scan_once(&dirs.state).await);

    let series_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let series_json = body_json(series_res).await;
    assert_eq!(series_json["total"], 0);
}

#[tokio::test]
async fn cover_generated_and_cached() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_cbz(
        dirs.books_dir.path(),
        "KapakSeri/kapakli.cbz",
        &[("01.png", png(600, 900, [120, 40, 200]))],
    );
    assert!(scan_once(&dirs.state).await);

    let series_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let series_json = body_json(series_res).await;
    let cover_book_id = series_json["items"][0]["cover_book_id"].as_i64().unwrap();

    let cover_file = dirs
        .data_dir
        .path()
        .join("covers")
        .join(format!("{cover_book_id}.jpg"));
    assert!(cover_file.exists());

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{cover_book_id}/cover"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg"
    );
    assert_eq!(
        res.headers().get(header::CACHE_CONTROL).unwrap(),
        "private, max-age=86400"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let img = image::load_from_memory(&body).unwrap();
    assert_eq!(img.width(), 300);
    assert_eq!(img.height(), 450);
}

#[tokio::test]
async fn non_utf8_name_skipped() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let valid_cbz = make_cbz(
        dirs.books_dir.path(),
        "gecerli.cbz",
        &[("p1.png", png(20, 20, [10, 20, 30]))],
    );
    let raw_bytes = std::fs::read(&valid_cbz).unwrap();

    let bad_name = OsStr::from_bytes(b"bozuk\xff.cbz");
    let bad_path = dirs.books_dir.path().join(bad_name);
    if std::fs::write(&bad_path, &raw_bytes).is_err() {
        return;
    }

    assert!(scan_once(&dirs.state).await);

    let series_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let series_json = body_json(series_res).await;
    assert_eq!(series_json["total"], 1);
    assert_eq!(series_json["items"][0]["book_count"], 1);

    let series_id = series_json["items"][0]["id"].as_i64().unwrap();
    let detail_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/series/{series_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail = body_json(detail_res).await;
    let books = detail["books"].as_array().unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0]["title"], "gecerli");
}
