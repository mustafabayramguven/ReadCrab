mod common;

use axum::http::StatusCode;
use common::{call, create_user_cookie, login_as_admin, seed_books, test_app, user_id};

async fn exec(dirs: &common::TempDirs, sql: String) {
    dirs.db
        .run(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .unwrap();
}

#[tokio::test]
async fn bookmarks_lifecycle() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let other = create_user_cookie(&app, &admin, "diger").await;
    let other_id = user_id(&dirs, "diger").await;
    let book = seed_books(&dirs, "S", 2).await[0];
    let uri = format!("/api/books/{book}/bookmarks");

    // yetkisiz
    let (st, _) = call(&app, "GET", &uri, "", None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    let body = r#"{"locator":"{\"ch\":1,\"p\":0.5}","label":"  Güzel yer  "}"#;
    let (st, v) = call(&app, "POST", &uri, &admin, Some(body)).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(v["label"], "Güzel yer");
    let id = v["id"].as_i64().unwrap();

    let (_, v) = call(&app, "GET", &uri, &admin, None).await;
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["locator"], r#"{"ch":1,"p":0.5}"#);
    // başkasının listesi boş
    let (st, v) = call(&app, "GET", &uri, &other, None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["items"].as_array().unwrap().is_empty());

    // başkası silemez, sahibi siler
    let del = format!("/api/bookmarks/{id}");
    let (st, _) = call(&app, "DELETE", &del, &other, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = call(&app, "DELETE", &del, &admin, None).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = call(&app, "DELETE", &del, &admin, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // geçersiz gövdeler
    let long_loc = "x".repeat(201);
    let long_label = "y".repeat(121);
    let (ll, lb) = (long_loc.as_str(), long_label.as_str());
    let cases = [("", "a"), (ll, "a"), ("1", "  "), ("1", lb)];
    for (loc, label) in cases {
        let b = format!(r#"{{"locator":"{loc}","label":"{label}"}}"#);
        let (st, _) = call(&app, "POST", &uri, &admin, Some(&b)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{loc}/{label}");
    }

    // eksik kitap
    let (st, _) = call(&app, "GET", "/api/books/9999/bookmarks", &admin, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let missing = seed_books(&dirs, "M", 1).await[0];
    let sql = format!("UPDATE books SET missing=1 WHERE id={missing}");
    exec(&dirs, sql).await;
    let uri_m = format!("/api/books/{missing}/bookmarks");
    let (st, _) = call(&app, "GET", &uri_m, &admin, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let ok_body = r#"{"locator":"1","label":"a"}"#;
    let (st, _) = call(&app, "POST", &uri_m, &admin, Some(ok_body)).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // 100 sınırı: 101. kayıt 409
    let b = r#"{"locator":"3","label":"a"}"#;
    for _ in 0..100 {
        let (st, _) = call(&app, "POST", &uri, &admin, Some(b)).await;
        assert_eq!(st, StatusCode::CREATED);
    }
    let (st, _) = call(&app, "POST", &uri, &admin, Some(b)).await;
    assert_eq!(st, StatusCode::CONFLICT);

    // kullanıcı silinince CASCADE
    call(&app, "POST", &uri, &other, Some(b)).await;
    let count = |dirs: &common::TempDirs| {
        let db = dirs.db.clone();
        async move {
            db.run(move |c| {
                let sql = "SELECT COUNT(*) FROM bookmarks WHERE user_id = ?1";
                Ok(c.query_row(sql, [other_id], |r| r.get::<_, i64>(0))?)
            })
            .await
            .unwrap()
        }
    };
    assert_eq!(count(&dirs).await, 1);
    exec(&dirs, format!("DELETE FROM users WHERE id = {other_id}")).await;
    assert_eq!(count(&dirs).await, 0);
}
