mod common;

use axum::http::StatusCode;
use common::{
    call, create_user_cookie, login_as_admin, make_cbz, png, raw, test_app, TempDirs,
};
use readcrab::scan::scan_once;

async fn book_id(dirs: &TempDirs, rel: &str) -> i64 {
    let rel = rel.to_string();
    dirs.db
        .run(move |c| {
            let sql = "SELECT id FROM books WHERE rel_path = ?1";
            Ok(c.query_row(sql, [rel], |r| r.get(0))?)
        })
        .await
        .unwrap()
}

async fn flags(dirs: &TempDirs, id: i64) -> (i64, i64, i64) {
    dirs.db
        .run(move |c| {
            let sql = "SELECT has_cover, custom_cover, cover_version FROM books WHERE id = ?1";
            Ok(c.query_row(sql, [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)
        })
        .await
        .unwrap()
}

async fn put(app: &axum::Router, cookie: &str, id: i64, ct: &str, body: Vec<u8>) -> StatusCode {
    let uri = format!("/api/books/{id}/cover");
    raw(app, "PUT", &uri, cookie, ct, None, body).await.0
}

async fn cover_width(app: &axum::Router, cookie: &str, id: i64) -> Option<u32> {
    let uri = format!("/api/books/{id}/cover");
    let (st, bytes) = raw(app, "GET", &uri, cookie, "", None, Vec::new()).await;
    if st != StatusCode::OK {
        return None;
    }
    assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    Some(image::load_from_memory(&bytes).unwrap().width())
}

#[tokio::test]
async fn custom_cover_lifecycle() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let user = create_user_cookie(&app, &admin, "diger").await;
    let books = dirs.books_dir.path();
    let page = png(20, 30, [1, 2, 3]);
    make_cbz(books, "Seri/1.cbz", &[("p1.png", page.clone())]);
    make_cbz(books, "Seri/2.cbz", &[("p1.png", page.clone())]);
    make_cbz(books, "Seri/bad.cbz", &[("p1.png", b"bozuk".to_vec())]);
    assert!(scan_once(&dirs.state).await);
    let b1 = book_id(&dirs, "Seri/1.cbz").await;
    let b2 = book_id(&dirs, "Seri/2.cbz").await;
    let bad = book_id(&dirs, "Seri/bad.cbz").await;
    assert_eq!(flags(&dirs, bad).await.0, 0);

    // yönetici dışı: 403
    let img = png(800, 1200, [200, 10, 10]);
    assert_eq!(
        put(&app, &user, b2, "image/png", img.clone()).await,
        StatusCode::FORBIDDEN
    );
    let uri = format!("/api/books/{b2}/cover");
    let (st, _) = call(&app, "DELETE", &uri, &user, None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // yükle: 600 genişlik, sürüm 1, seri kapağı özel kapaklı kitap
    assert_eq!(cover_width(&app, &admin, b2).await, Some(300));
    assert_eq!(put(&app, &admin, b2, "image/png", img).await, StatusCode::OK);
    assert_eq!(cover_width(&app, &admin, b2).await, Some(600));
    assert_eq!(flags(&dirs, b2).await, (1, 1, 1));
    let (_, v) = call(&app, "GET", "/api/series", &admin, None).await;
    assert_eq!(v["items"][0]["cover_book_id"], b2);
    assert_eq!(v["items"][0]["cover_v"], 1);
    let (_, v) = call(&app, "GET", &format!("/api/books/{b2}"), &admin, None).await;
    assert_eq!(v["cover_version"], 1);

    // geçersiz girdiler
    let ok_png = png(10, 10, [1, 1, 1]);
    let bad_cases: [(&str, Vec<u8>, StatusCode); 6] = [
        ("image/png", b"bozuk".to_vec(), StatusCode::BAD_REQUEST),
        ("image/png", b"bu bir metin".to_vec(), StatusCode::BAD_REQUEST),
        ("image/png", png(16385, 1, [0, 0, 0]), StatusCode::BAD_REQUEST),
        ("image/png", vec![0; 11 * 1024 * 1024], StatusCode::PAYLOAD_TOO_LARGE),
        ("image/gif", ok_png.clone(), StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("text/plain", ok_png, StatusCode::UNSUPPORTED_MEDIA_TYPE),
    ];
    for (ct, body, want) in bad_cases {
        assert_eq!(put(&app, &admin, b2, ct, body).await, want, "{ct}");
    }
    assert_eq!(flags(&dirs, b2).await, (1, 1, 1));
    assert_eq!(
        put(&app, &admin, 9999, "image/png", png(5, 5, [1, 1, 1])).await,
        StatusCode::NOT_FOUND
    );

    // yeniden tarama özel kapağı ezmez (dosya değişse de)
    let before = std::fs::read(dirs.data_dir.path().join(format!("covers/{b2}.jpg"))).unwrap();
    make_cbz(books, "Seri/2.cbz", &[("p1.png", page.clone()), ("p2.png", page)]);
    assert!(scan_once(&dirs.state).await);
    let after = std::fs::read(dirs.data_dir.path().join(format!("covers/{b2}.jpg"))).unwrap();
    assert_eq!(before, after);
    assert_eq!(flags(&dirs, b2).await, (1, 1, 1));

    // sil: çıkarılmış kapağa dön, sürüm 2
    let (st, v) = call(&app, "DELETE", &uri, &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["cover_version"], 2);
    assert_eq!(v["has_cover"], true);
    assert_eq!(flags(&dirs, b2).await, (1, 0, 2));
    assert_eq!(cover_width(&app, &admin, b2).await, Some(300));
    let (_, v) = call(&app, "GET", "/api/series", &admin, None).await;
    assert_eq!(v["items"][0]["cover_book_id"], b1);

    // kapaksız kitapta: yükle sonra sil -> has_cover=0
    let png_ok = png(50, 50, [9, 9, 9]);
    assert_eq!(put(&app, &admin, bad, "image/png", png_ok).await, StatusCode::OK);
    assert_eq!(flags(&dirs, bad).await, (1, 1, 1));
    let (st, v) = call(&app, "DELETE", &format!("/api/books/{bad}/cover"), &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["has_cover"], false);
    assert_eq!(flags(&dirs, bad).await, (0, 0, 2));
    assert_eq!(cover_width(&app, &admin, bad).await, None);
}
