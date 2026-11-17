mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::{body_json, login_as_admin, make_epub, test_app, EpubOpts};
use readcrab::scan::scan_once;
use tower::ServiceExt;

const PHONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LAPTOP: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

async fn put(
    app: &axum::Router,
    cookie: &str,
    id: i64,
    body: String,
) -> (StatusCode, serde_json::Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/books/{id}/progress"))
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (status, body_json(res).await)
}

fn save(locator: &str, percent: f64, client: &str, base: i64) -> String {
    serde_json::json!({
        "locator": locator, "percent": percent, "local_date": "2026-11-17",
        "client_id": client, "base_updated_at": base
    })
    .to_string()
}

async fn set_updated_at(dirs: &common::TempDirs, id: i64, at: i64) {
    dirs.db
        .run(move |conn| {
            conn.execute(
                "UPDATE progress SET updated_at = ?1 WHERE book_id = ?2",
                rusqlite::params![at, id],
            )?;
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn stale_tab_cannot_overwrite_newer_position_from_another_device() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    make_epub(
        dirs.books_dir.path(),
        "Seri/kitap.epub",
        EpubOpts::default(),
    );
    assert!(scan_once(&dirs.state).await);
    let id: i64 = dirs
        .db
        .run(|conn| Ok(conn.query_row("SELECT id FROM books", [], |r| r.get(0))?))
        .await
        .unwrap();

    // Telefon okur ve kaydeder.
    let (st, phone) = put(
        &app,
        &cookie,
        id,
        save(r#"{"ch":0,"p":0.2,"t":0.2}"#, 0.1, PHONE, 0),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let phone_at = phone["updated_at"].as_i64().unwrap();
    assert!(phone_at > 0);
    // Dizüstü, telefonun konumunu gördükten sonra ilerler.
    set_updated_at(&dirs, id, phone_at - 10).await;
    let (st, laptop) = put(
        &app,
        &cookie,
        id,
        save(r#"{"ch":2,"p":0.5,"t":0.5}"#, 0.6, LAPTOP, phone_at - 10),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let laptop_at = laptop["updated_at"].as_i64().unwrap();

    // Telefonda açık kalan eski sekme, eski temeliyle yazamaz; güncel kayıt döner.
    set_updated_at(&dirs, id, laptop_at + 5).await;
    let (st, conflict) = put(
        &app,
        &cookie,
        id,
        save(r#"{"ch":0,"p":0.25,"t":0.25}"#, 0.12, PHONE, phone_at),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(conflict["error"], "conflict");
    assert_eq!(
        conflict["progress"]["locator"],
        r#"{"ch":2,"p":0.5,"t":0.5}"#
    );
    assert_eq!(conflict["progress"]["client_id"], LAPTOP);
    let latest = conflict["progress"]["updated_at"].as_i64().unwrap();

    // "Burada kal": güncel kaydı temel alarak bilerek yazar.
    let (st, _) = put(
        &app,
        &cookie,
        id,
        save(r#"{"ch":0,"p":0.25,"t":0.25}"#, 0.12, PHONE, latest),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // Aynı sekme kendi yazdığının üstüne her zaman yazabilir.
    let (st, _) = put(
        &app,
        &cookie,
        id,
        save(r#"{"ch":0,"p":0.3,"t":0.3}"#, 0.13, PHONE, 0),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // Kimlik göndermeyen eski istemciler eskisi gibi çalışır.
    let (st, _) = put(
        &app,
        &cookie,
        id,
        r#"{"locator":"{\"ch\":1,\"p\":0}","percent":0.3,"local_date":"2026-11-17"}"#.to_string(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let book = body_json(res).await;
    assert!(book["progress_updated_at"].as_i64().unwrap() > 0);

    let (st, _) = put(&app, &cookie, id, save("{}", 0.1, "not-hex", 0)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}
