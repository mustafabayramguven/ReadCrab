mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::{
    body_json, login_as_admin, make_cbz, make_epub, make_epub_drm, make_epub_malicious, make_mobi,
    png, test_app, EpubOpts,
};
use http_body_util::BodyExt;
use kavita_rs::{formats::sanitize::FRAME_CSP, scan::scan_once};
use tower::ServiceExt;

async fn book_id_by_rel(dirs: &common::TempDirs, rel: &str) -> i64 {
    let rel = rel.to_string();
    dirs.db
        .run(move |conn| {
            conn.query_row(
                "SELECT id FROM books WHERE rel_path = ?1",
                rusqlite::params![rel],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn cbz_pages_and_page_bytes() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    let p1 = png(16, 16, [200, 10, 10]);
    let p2 = png(16, 16, [10, 200, 10]);
    make_cbz(
        dirs.books_dir.path(),
        "Cizgi/test.cbz",
        &[("p10.png", p2.clone()), ("p2.png", p1.clone())],
    );
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "Cizgi/test.cbz").await;

    let pages_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/pages"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pages_res.status(), StatusCode::OK);
    let pages_json = body_json(pages_res).await;
    assert_eq!(pages_json["count"], 2);

    let p0_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/pages/0"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(p0_res.status(), StatusCode::OK);
    assert_eq!(
        p0_res.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
    assert_eq!(
        p0_res
            .headers()
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .unwrap(),
        "nosniff"
    );
    assert_eq!(
        p0_res.headers().get(header::CACHE_CONTROL).unwrap(),
        "private, max-age=3600"
    );
    let p0_bytes = p0_res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&p0_bytes[..], &p1[..]);

    let oob_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/pages/99"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oob_res.status(), StatusCode::NOT_FOUND);

    let toc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/toc"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(toc_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cbz_corrupt_returns_422() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    std::fs::write(dirs.books_dir.path().join("bozuk.cbz"), b"not a zip file").unwrap();
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "bozuk.cbz").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/pages"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"], "corrupt_book");
}

#[tokio::test]
async fn epub_toc_and_chapter() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub(
        dirs.books_dir.path(),
        "kitap.epub",
        EpubOpts {
            title: Some("Örnek EPUB".to_string()),
            svg_cover: false,
        },
    );
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "kitap.epub").await;

    let toc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/toc"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(toc_res.status(), StatusCode::OK);
    let toc = body_json(toc_res).await;
    let chapters = toc["chapters"].as_array().unwrap();
    assert_eq!(chapters.len(), 3);
    assert_eq!(chapters[0]["index"], 0);
    assert_eq!(chapters[0]["title"], "Bölüm Bir");

    let ch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/chapters/0"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ch_res.status(), StatusCode::OK);
    assert_eq!(
        ch_res.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        ch_res
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap(),
        FRAME_CSP
    );
    let html_bytes = ch_res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(html_bytes.to_vec()).unwrap();
    assert!(html.contains("Birinci bölüm metni"));
    assert!(html.contains("/static/frame.css"));
}

#[tokio::test]
async fn epub_asset_only_manifest_images() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub(dirs.books_dir.path(), "kitap.epub", EpubOpts::default());
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "kitap.epub").await;

    let ok_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/asset?p=OEBPS/images/pic.png"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_res.status(), StatusCode::OK);
    assert_eq!(
        ok_res.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
    assert_eq!(
        ok_res
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap(),
        "sandbox; default-src 'none'"
    );
    assert_eq!(
        ok_res
            .headers()
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .unwrap(),
        "nosniff"
    );

    for forbidden in [
        "OEBPS/images/unlisted.png",
        "OEBPS/secret.txt",
        "OEBPS/ch1.xhtml",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/books/{id}/asset?p={forbidden}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{forbidden}");
    }
}

#[tokio::test]
async fn epub_asset_path_traversal_404() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub(dirs.books_dir.path(), "kitap.epub", EpubOpts::default());
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "kitap.epub").await;

    for attempt in [
        "../../etc/passwd",
        "%2e%2e/%2e%2e/etc/passwd",
        "OEBPS/../../etc/passwd",
        "/etc/passwd",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/books/{id}/asset?p={attempt}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{attempt}");
    }
}

#[tokio::test]
async fn epub_drm_returns_422_drm_protected() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub_drm(dirs.books_dir.path());
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "drm.epub").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/toc"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"], "drm_protected");
}

#[tokio::test]
async fn epub_sanitizes_xss() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub_malicious(dirs.books_dir.path());
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "malicious.epub").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/chapters/0"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let csp = res
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(csp, FRAME_CSP);
    assert!(!csp.contains("sandbox"));

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!html.contains("<script"));
    assert!(!html.contains("onerror"));
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("<svg"));
    assert!(html.contains("Temiz içerik"));
}

#[tokio::test]
async fn epub_svg_cover_becomes_img() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_epub(
        dirs.books_dir.path(),
        "svg.epub",
        EpubOpts {
            title: None,
            svg_cover: true,
        },
    );
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "svg.epub").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/chapters/0"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!html.contains("<svg"));

    let expected_url = format!("/api/books/{id}/asset?p=OEBPS%2Fimages%2Fcover%2Epng");
    assert!(html.contains(&expected_url), "html: {html}");

    let asset_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&expected_url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(asset_res.status(), StatusCode::OK);
    let asset_bytes = asset_res.into_body().collect().await.unwrap().to_bytes();
    assert!(asset_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[tokio::test]
async fn mobi_chapters_and_images() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_mobi(dirs.books_dir.path(), "ornek.mobi");
    assert!(scan_once(&dirs.state).await);
    let id = book_id_by_rel(&dirs, "ornek.mobi").await;

    let toc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/toc"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(toc_res.status(), StatusCode::OK);
    let toc = body_json(toc_res).await;
    assert_eq!(toc["chapters"].as_array().unwrap().len(), 2);

    let ch1_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/books/{id}/chapters/1"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ch1_res.status(), StatusCode::OK);
    let ch1_bytes = ch1_res.into_body().collect().await.unwrap().to_bytes();
    let ch1_html = String::from_utf8(ch1_bytes.to_vec()).unwrap();
    let asset_url = format!("/api/books/{id}/asset?p=1");
    assert!(ch1_html.contains(&asset_url), "ch1_html: {ch1_html}");

    let asset_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&asset_url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(asset_res.status(), StatusCode::OK);
    let asset_bytes = asset_res.into_body().collect().await.unwrap().to_bytes();
    assert!(asset_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}
