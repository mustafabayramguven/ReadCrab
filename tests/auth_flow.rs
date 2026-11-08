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

    let endpoints = [("GET", "/api/me"), ("POST", "/api/logout")];
    for (method, uri) in endpoints {
        let mut builder = Request::builder().method(method).uri(uri);
        let body = if method == "POST" {
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
