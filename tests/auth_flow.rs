mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

use common::{body_json, login_as_admin, test_app, test_app_with_secure};

#[tokio::test]
async fn setup_creates_first_admin_then_closes() {
    let (app, _dirs) = test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/setup-status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["needs_setup"], true);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let json = body_json(res).await;
    assert_eq!(json["username"], "admin");
    assert_eq!(json["is_admin"], true);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"second","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let json = body_json(res).await;
    assert_eq!(json["error"], "conflict");
}

#[tokio::test]
async fn unauthenticated_endpoints_return_401() {
    let (app, _dirs) = test_app().await;
    let _cookie = login_as_admin(&app).await;

    let endpoints = [
        ("POST", "/api/logout"),
        ("GET", "/api/me"),
        ("PUT", "/api/me/password"),
        ("GET", "/api/scan-status"),
        ("GET", "/api/series"),
        ("GET", "/api/series/1"),
        ("GET", "/api/books/1"),
        ("GET", "/api/books/1/cover"),
        ("GET", "/api/books/1/pages"),
        ("GET", "/api/books/1/pages/0"),
        ("GET", "/api/books/1/toc"),
        ("GET", "/api/books/1/chapters/0"),
        ("GET", "/api/books/1/asset?p=1"),
        ("GET", "/api/books/1/progress"),
        ("PUT", "/api/books/1/progress"),
        ("PUT", "/api/books/1/status"),
        ("POST", "/api/reading/ping"),
        ("GET", "/api/history"),
        ("GET", "/api/stats?today=2026-11-08"),
        ("PUT", "/api/goal"),
        ("GET", "/api/admin/users"),
        ("POST", "/api/admin/users"),
        ("DELETE", "/api/admin/users/1"),
        ("PUT", "/api/admin/users/1/password"),
        ("POST", "/api/admin/scan"),
    ];
    for (method, uri) in endpoints {
        let mut builder = Request::builder().method(method).uri(uri);
        let body = if method != "GET" {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from("{}")
        } else {
            Body::empty()
        };
        let req = builder.body(body).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn cookie_flags() {
    let (app, _dirs) = test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let sc = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(sc.contains("HttpOnly"));
    assert!(sc.contains("SameSite=Strict"));
    assert!(!sc.contains("Secure"));

    let (app_sec, _dirs_sec) = test_app_with_secure(true).await;
    let res_sec = app_sec
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_sec.status(), StatusCode::CREATED);
    let sc_sec = res_sec
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(sc_sec.contains("HttpOnly"));
    assert!(sc_sec.contains("SameSite=Strict"));
    assert!(sc_sec.contains("Secure"));
}

#[tokio::test]
async fn password_is_argon2() {
    let (app, dirs) = test_app().await;
    let _cookie = login_as_admin(&app).await;

    let hash: String = dirs
        .db
        .run(|conn| {
            conn.query_row("SELECT password_hash FROM users LIMIT 1", [], |r| r.get(0))
                .map_err(Into::into)
        })
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"), "hash: {hash}");
}

#[tokio::test]
async fn login_rate_limit() {
    let (app, _dirs) = test_app().await;
    let _cookie = login_as_admin(&app).await;

    for _ in 0..5 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"admin","password":"wrongpassword"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    let res6 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"wrongpassword"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res6.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(res6.headers().contains_key(header::RETRY_AFTER));

    let res_correct = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"adminpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_correct.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn csrf_origin_mismatch_403() {
    let (app, _dirs) = test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::HOST, "localhost:3435")
                .header(header::ORIGIN, "http://evil.example")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let json = body_json(res).await;
    assert_eq!(json["error"], "csrf");
}

#[tokio::test]
async fn post_requires_json_content_type_415() {
    let (app, _dirs) = test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from(
                    r#"{"username":"admin","password":"password1234"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let json = body_json(res).await;
    assert_eq!(json["error"], "unsupported_media");
}

#[tokio::test]
async fn non_admin_gets_403_on_admin_routes() {
    let (app, _dirs) = test_app().await;
    let admin_cookie = login_as_admin(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"normaluser","password":"userpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = body_json(res).await;
    assert_eq!(created["is_admin"], false);

    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"normaluser","password":"userpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    let user_cookie = login_res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let routes = [
        ("GET", "/api/admin/users", ""),
        (
            "POST",
            "/api/admin/users",
            r#"{"username":"u2","password":"password1234"}"#,
        ),
        ("DELETE", "/api/admin/users/1", "{}"),
        (
            "PUT",
            "/api/admin/users/1/password",
            r#"{"password":"newpassword123"}"#,
        ),
        ("POST", "/api/admin/scan", "{}"),
    ];

    for (method, uri, body_str) in routes {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, &user_cookie);
        let body = if method != "GET" {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(body_str)
        } else {
            Body::empty()
        };
        let req = builder.body(body).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

#[tokio::test]
async fn delete_user_removes_data() {
    let (app, dirs) = test_app().await;
    let admin_cookie = login_as_admin(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"reader","password":"readerpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let reader_json = body_json(res).await;
    let reader_id = reader_json["id"].as_i64().unwrap();

    dirs.db
        .run(move |conn| {
            conn.execute(
                "INSERT INTO series(id, rel_path, name, added_at) VALUES (1, 's', 'S', 100)",
                [],
            )?;
            conn.execute(
                "INSERT INTO books(id, series_id, rel_path, title, format, size, mtime, content_id, added_at)
                 VALUES (1, 1, 's/b.cbz', 'B', 'cbz', 10, 100, 'cid', 100)",
                [],
            )?;
            conn.execute(
                "INSERT INTO progress(user_id, book_id, locator, percent, status, updated_at)
                 VALUES (?1, 1, '1', 0.5, 'reading', 100)",
                rusqlite::params![reader_id],
            )?;
            conn.execute(
                "INSERT INTO reading_sessions(user_id, book_id, client_id, started_at, ended_at, active_seconds, pages_read, start_percent, end_percent, local_date)
                 VALUES (?1, 1, '00000000000000000000000000000001', 100, 160, 60, 5, 0.0, 0.5, '2026-11-08')",
                rusqlite::params![reader_id],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let self_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/admin/users/1")
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(self_del.status(), StatusCode::CONFLICT);
    let self_json = body_json(self_del).await;
    assert_eq!(self_json["message"], "cannot_delete_self");

    let del_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/admin/users/{reader_id}"))
                .header(header::COOKIE, &admin_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_res.status(), StatusCode::NO_CONTENT);

    let (prog_cnt, sess_cnt): (i64, i64) = dirs
        .db
        .run(move |conn| {
            let p: i64 = conn.query_row(
                "SELECT COUNT(*) FROM progress WHERE user_id = ?1",
                rusqlite::params![reader_id],
                |r| r.get(0),
            )?;
            let s: i64 = conn.query_row(
                "SELECT COUNT(*) FROM reading_sessions WHERE user_id = ?1",
                rusqlite::params![reader_id],
                |r| r.get(0),
            )?;
            Ok((p, s))
        })
        .await
        .unwrap();
    assert_eq!(prog_cnt, 0);
    assert_eq!(sess_cnt, 0);
}

#[tokio::test]
async fn change_own_password() {
    let (app, _dirs) = test_app().await;
    let cookie1 = login_as_admin(&app).await;

    let login2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"adminpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login2.status(), StatusCode::OK);
    let cookie2 = login2
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let bad_short = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/me/password")
                .header(header::COOKIE, &cookie1)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"adminpassword123","new_password":"short"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_short.status(), StatusCode::BAD_REQUEST);

    let bad_cur = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/me/password")
                .header(header::COOKIE, &cookie1)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"wrongpassword123","new_password":"newpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_cur.status(), StatusCode::UNAUTHORIZED);

    let ok_change = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/me/password")
                .header(header::COOKIE, &cookie1)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"adminpassword123","new_password":"newpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_change.status(), StatusCode::NO_CONTENT);

    let me2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header(header::COOKIE, &cookie2)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me2.status(), StatusCode::UNAUTHORIZED);

    let me1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header(header::COOKIE, &cookie1)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me1.status(), StatusCode::OK);

    let old_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"adminpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"newpassword123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
}
