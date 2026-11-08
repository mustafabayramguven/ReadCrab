mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::{body_json, login_as_admin, make_cbz, make_epub, png, test_app, EpubOpts};
use kavita_rs::scan::scan_once;
use tower::ServiceExt;

async fn setup_two_books(dirs: &common::TempDirs) -> (i64, i64) {
    let img = png(20, 20, [10, 20, 30]);
    make_cbz(
        dirs.books_dir.path(),
        "Seri/cizgi.cbz",
        &[("p1.png", img.clone()), ("p2.png", img)],
    );
    make_epub(
        dirs.books_dir.path(),
        "Seri/metin.epub",
        EpubOpts::default(),
    );
    assert!(scan_once(&dirs.state).await);

    dirs.db
        .run(|conn| {
            let cbz_id: i64 = conn.query_row(
                "SELECT id FROM books WHERE rel_path = 'Seri/cizgi.cbz'",
                [],
                |r| r.get(0),
            )?;
            let epub_id: i64 = conn.query_row(
                "SELECT id FROM books WHERE rel_path = 'Seri/metin.epub'",
                [],
                |r| r.get(0),
            )?;
            Ok((cbz_id, epub_id))
        })
        .await
        .unwrap()
}

async fn create_and_login_user(app: &axum::Router, admin_cookie: &str, username: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header(header::COOKIE, admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"{username}","password":"password1234"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"{username}","password":"password1234"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    login_res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn progress_roundtrip_comic_and_text() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, epub_id) = setup_two_books(&dirs).await;

    let init_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(init_res.status(), StatusCode::OK);
    let init_json = body_json(init_res).await;
    assert_eq!(init_json["locator"], "");
    assert_eq!(init_json["percent"], 0.0);
    assert_eq!(init_json["status"], "unread");

    let put_cbz = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"1","percent":0.5,"local_date":"2026-11-08"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_cbz.status(), StatusCode::OK);

    let get_cbz = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cbz_json = body_json(get_cbz).await;
    assert_eq!(cbz_json["locator"], "1");
    assert_eq!(cbz_json["percent"], 0.5);
    assert_eq!(cbz_json["status"], "reading");

    let put_epub = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{epub_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"{\"ch\":2,\"p\":0.42}","percent":0.8,"local_date":"2026-11-08"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_epub.status(), StatusCode::OK);

    let get_epub = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{epub_id}/progress"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let epub_json = body_json(get_epub).await;
    assert_eq!(epub_json["locator"], r#"{"ch":2,"p":0.42}"#);
    assert_eq!(epub_json["percent"], 0.8);
    assert_eq!(epub_json["status"], "reading");
}

#[tokio::test]
async fn status_rules() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    // Rule 3: unread -> reading when percent < 0.99
    let r1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"0","percent":0.4,"local_date":"2026-10-01"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(r1).await["status"], "reading");

    // Rule 1: percent >= 0.99 -> finished, sets finished_date
    let r2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"1","percent":0.995,"local_date":"2026-10-02"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(r2).await["status"], "finished");

    // Rule 2: re-reading (percent < 0.99) when already finished keeps finished and original finished_date
    let r3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"0","percent":0.2,"local_date":"2026-10-05"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(r3).await["status"], "finished");

    let fin_date: Option<String> = dirs
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT finished_date FROM progress WHERE user_id = 1 AND book_id = ?1",
                [cbz_id],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .unwrap();
    assert_eq!(fin_date.as_deref(), Some("2026-10-02"));

    // Calling PUT status -> finished when already finished preserves finished_date
    let r_fin_again = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/status"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"status":"finished","local_date":"2026-10-15"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r_fin_again.status(), StatusCode::OK);

    let fin_date_kept: Option<String> = dirs
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT finished_date FROM progress WHERE user_id = 1 AND book_id = ?1",
                [cbz_id],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .unwrap();
    assert_eq!(fin_date_kept.as_deref(), Some("2026-10-02"));

    // Book and series endpoints return percent = 1.0 when status is finished
    let book_fin_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{cbz_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let book_fin_json = body_json(book_fin_res).await;
    assert_eq!(book_fin_json["status"], "finished");
    assert_eq!(book_fin_json["percent"], 1.0);

    // Manual PUT status -> dropped (clears finished_date)
    let r4 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/status"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"status":"dropped","local_date":"2026-10-06"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(r4).await["status"], "dropped");

    let fin_date_after_drop: Option<String> = dirs
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT finished_date FROM progress WHERE user_id = 1 AND book_id = ?1",
                [cbz_id],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .unwrap();
    assert!(fin_date_after_drop.is_none());

    // Subsequent PUT progress re-evaluates: dropped -> reading
    let r5 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/progress"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"locator":"0","percent":0.3,"local_date":"2026-10-07"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(r5).await["status"], "reading");
}

#[tokio::test]
async fn ping_upsert_monotonic_and_clamped() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, _) = setup_two_books(&dirs).await;
    let cid = "0123456789abcdef0123456789abcdef";

    // First ping: active_seconds=30, pages_read=4
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"{cid}","book_id":{cbz_id},"active_seconds":30,"pages_read":4,"start_percent":0.1,"end_percent":0.3,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::NO_CONTENT);

    // Second ping with smaller active_seconds=10, smaller pages_read=2 -> monotonic keeps 30 and 4
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"{cid}","book_id":{cbz_id},"active_seconds":10,"pages_read":2,"start_percent":0.9,"end_percent":0.5,"local_date":"2026-11-09"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::NO_CONTENT);

    // Third ping with huge active_seconds=50000 -> clamped to (now - started_at) + 5 (~35..37s)
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"{cid}","book_id":{cbz_id},"active_seconds":50000,"pages_read":9,"start_percent":0.9,"end_percent":0.8,"local_date":"2026-11-09"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res3.status(), StatusCode::NO_CONTENT);

    let hist_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/history")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let hist = body_json(hist_res).await;
    assert_eq!(hist["total"], 1);
    let item = &hist["items"][0];
    let active = item["active_seconds"].as_i64().unwrap();
    assert!((35..=40).contains(&active), "active: {active}");
    assert_eq!(item["pages_read"], 9);
    assert_eq!(item["start_percent"], 0.1);
    assert_eq!(item["end_percent"], 0.8);
    assert_eq!(item["local_date"], "2026-11-08");
}

#[tokio::test]
async fn ping_zero_creates_nothing() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","book_id":{cbz_id},"active_seconds":0,"pages_read":0,"start_percent":0.0,"end_percent":0.0,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","book_id":{cbz_id},"active_seconds":0,"pages_read":1,"start_percent":0.0,"end_percent":0.5,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::NO_CONTENT);

    let hist_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/history")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let hist = body_json(hist_res).await;
    assert_eq!(hist["total"], 0);
}

#[tokio::test]
async fn history_only_own_rows() {
    let (app, dirs) = test_app().await;
    let admin_cookie = login_as_admin(&app).await;
    let user_cookie = create_and_login_user(&app, &admin_cookie, "okur1").await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    let ping_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"11111111111111111111111111111111","book_id":{cbz_id},"active_seconds":120,"pages_read":5,"start_percent":0.0,"end_percent":0.5,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ping_res.status(), StatusCode::NO_CONTENT);

    let user_hist_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/history")
                .header(header::COOKIE, &user_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let user_hist = body_json(user_hist_res).await;
    assert_eq!(user_hist["total"], 0);
}

#[tokio::test]
async fn stats_streaks_and_daily() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    dirs.db
        .run(move |conn| {
            let sessions = [
                // Longest streak of 3 days in October: Oct 10, 11, 12 (>= 60s each)
                ("c0000000000000000000000000000001", "2026-10-10", 60, 2),
                ("c0000000000000000000000000000002", "2026-10-11", 90, 3),
                ("c0000000000000000000000000000003", "2026-10-12", 120, 4),
                // Day with < 60s does not count towards streak:
                ("c0000000000000000000000000000004", "2026-10-13", 59, 1),
                // Current streak ending yesterday (Nov 6, Nov 7) when today = Nov 8:
                ("c0000000000000000000000000000005", "2026-11-06", 30, 1),
                ("c0000000000000000000000000000006", "2026-11-06", 35, 2), // sum = 65 >= 60
                ("c0000000000000000000000000000007", "2026-11-07", 100, 5),
            ];
            for (cid, date, secs, pages) in sessions {
                conn.execute(
                    "INSERT INTO reading_sessions(
                        user_id, book_id, client_id, started_at, ended_at,
                        active_seconds, pages_read, start_percent, end_percent, local_date
                     ) VALUES (1, ?1, ?2, 1000, 1100, ?3, ?4, 0.0, 0.5, ?5)",
                    rusqlite::params![cbz_id, cid, secs, pages, date],
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/stats?today=2026-11-08")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = body_json(res).await;

    assert_eq!(stats["total_seconds"], 60 + 90 + 120 + 59 + 30 + 35 + 100);
    assert_eq!(stats["total_pages"], 2 + 3 + 4 + 1 + 1 + 2 + 5);
    assert_eq!(stats["streak_longest"], 3);
    assert_eq!(stats["streak_current"], 2);

    let daily = stats["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 30);
    assert_eq!(daily[0]["date"], "2026-10-10");
    assert_eq!(daily[0]["seconds"], 60);
    assert_eq!(daily[29]["date"], "2026-11-08");
    assert_eq!(daily[29]["seconds"], 0);
    assert_eq!(daily[28]["date"], "2026-11-07");
    assert_eq!(daily[28]["seconds"], 100);
    assert_eq!(daily[27]["date"], "2026-11-06");
    assert_eq!(daily[27]["seconds"], 65);
}

#[tokio::test]
async fn stats_other_user_isolated() {
    let (app, dirs) = test_app().await;
    let admin_cookie = login_as_admin(&app).await;
    let user_cookie = create_and_login_user(&app, &admin_cookie, "izole_okur").await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    let ping_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"22222222222222222222222222222222","book_id":{cbz_id},"active_seconds":300,"pages_read":12,"start_percent":0.0,"end_percent":1.0,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ping_res.status(), StatusCode::NO_CONTENT);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/stats?today=2026-11-08")
                .header(header::COOKIE, &user_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let stats = body_json(res).await;
    assert_eq!(stats["total_seconds"], 0);
    assert_eq!(stats["total_pages"], 0);
    assert_eq!(stats["streak_current"], 0);
}

#[tokio::test]
async fn goal_roundtrip_and_done_count() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, epub_id) = setup_two_books(&dirs).await;

    let put_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/goal")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"year":2026,"target":12}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_res.status(), StatusCode::OK);
    let goal_json = body_json(put_res).await;
    assert_eq!(goal_json["year"], 2026);
    assert_eq!(goal_json["target"], 12);

    // Finish cbz_id in 2026 and epub_id in 2025
    let st1_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{cbz_id}/status"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"status":"finished","local_date":"2026-05-10"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(st1_res.status(), StatusCode::OK);
    let st2_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{epub_id}/status"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"status":"finished","local_date":"2025-12-31"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(st2_res.status(), StatusCode::OK);

    let stats_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/stats?today=2026-11-08")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let stats = body_json(stats_res).await;
    assert_eq!(stats["finished_count"], 2);
    assert_eq!(stats["goal"]["year"], 2026);
    assert_eq!(stats["goal"]["target"], 12);
    assert_eq!(stats["goal"]["done"], 1);
}

#[tokio::test]
async fn local_date_validation_400() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, _) = setup_two_books(&dirs).await;

    for bad_date in [
        "2026-02-29",
        "2026-13-01",
        "2026-00-10",
        "2026-04-31",
        "2026/11/08",
        "2026-1-08",
        "not-a-date",
    ] {
        let r1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/books/{cbz_id}/progress"))
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"locator":"0","percent":0.5,"local_date":"{bad_date}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r1.status(), StatusCode::BAD_REQUEST, "progress {bad_date}");

        let r2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/stats?today={bad_date}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r2.status(), StatusCode::BAD_REQUEST, "stats {bad_date}");
    }
}

#[tokio::test]
async fn ping_client_id_book_mismatch_409() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let (cbz_id, epub_id) = setup_two_books(&dirs).await;
    let cid = "abcdef0123456789abcdef0123456789";

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"{cid}","book_id":{cbz_id},"active_seconds":30,"pages_read":3,"start_percent":0.0,"end_percent":0.5,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::NO_CONTENT);

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reading/ping")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"client_id":"{cid}","book_id":{epub_id},"active_seconds":60,"pages_read":9,"start_percent":0.0,"end_percent":1.0,"local_date":"2026-11-08"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT);
    let err_json = body_json(res2).await;
    assert_eq!(err_json["error"], "conflict");

    let hist_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/history")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let hist = body_json(hist_res).await;
    assert_eq!(hist["total"], 1);
    assert_eq!(hist["items"][0]["book_id"], cbz_id);
    assert_eq!(hist["items"][0]["active_seconds"], 30);
    assert_eq!(hist["items"][0]["pages_read"], 3);
}
