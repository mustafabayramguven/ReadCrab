#![allow(dead_code)]

use std::{io::Cursor, sync::Arc};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use image::{ImageFormat, Rgb, RgbImage};
use kavita_rs::{
    app::{build_app, AppState},
    auth::LoginLimiter,
    config::Config,
    db::Db,
};
use tempfile::TempDir;
use tower::ServiceExt;

pub struct TempDirs {
    pub data_dir: TempDir,
    pub books_dir: TempDir,
    pub db: Db,
    pub state: AppState,
}

pub fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let img = RgbImage::from_pixel(w, h, Rgb(rgb));
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .unwrap();
    buf
}

pub async fn test_app_with_secure(cookie_secure: bool) -> (Router, TempDirs) {
    let data_dir = tempfile::tempdir().unwrap();
    let books_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(data_dir.path().join("covers")).unwrap();

    let db = Db::open(&data_dir.path().join("kavita.db")).unwrap();
    let cfg = Config {
        port: 3435,
        data_dir: data_dir.path().to_path_buf(),
        books_dir: books_dir.path().to_path_buf(),
        cookie_secure,
    };
    let state = AppState {
        db: db.clone(),
        cfg: Arc::new(cfg),
        limiter: Arc::new(LoginLimiter::new()),
    };
    let app = build_app(state.clone());
    (
        app,
        TempDirs {
            data_dir,
            books_dir,
            db,
            state,
        },
    )
}

pub async fn test_app() -> (Router, TempDirs) {
    test_app_with_secure(false).await
}

pub async fn login_as_admin(app: &Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/setup")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"adminpassword123"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    if res.status() == StatusCode::CREATED {
        let set_cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        return set_cookie.split(';').next().unwrap().to_string();
    }

    let req = Request::builder()
        .method("POST")
        .uri("/api/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"adminpassword123"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    set_cookie.split(';').next().unwrap().to_string()
}

pub async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
