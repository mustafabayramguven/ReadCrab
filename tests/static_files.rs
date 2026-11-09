mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use common::test_app;
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn get(app: &Router, uri: &str) -> Response {
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_bytes(res: Response) -> Vec<u8> {
    res.into_body().collect().await.unwrap().to_bytes().to_vec()
}

#[tokio::test]
async fn static_files_have_expected_types() {
    let (app, _dirs) = test_app().await;
    let expected: &[(&str, &str)] = &[
        ("app.js", "application/javascript; charset=utf-8"),
        ("tracker.js", "application/javascript; charset=utf-8"),
        ("reader-comic.js", "application/javascript; charset=utf-8"),
        ("reader-text.js", "application/javascript; charset=utf-8"),
        ("stats.js", "application/javascript; charset=utf-8"),
        ("theme.js", "application/javascript; charset=utf-8"),
        ("style.css", "text/css; charset=utf-8"),
        ("frame.css", "text/css; charset=utf-8"),
        ("favicon.svg", "image/svg+xml"),
        ("icons.svg", "image/svg+xml"),
        ("fonts/roboto-flex-latin.woff2", "font/woff2"),
        ("fonts/roboto-flex-latin-ext.woff2", "font/woff2"),
        ("calc.js", "application/javascript; charset=utf-8"),
        ("reader-ui.js", "application/javascript; charset=utf-8"),
        ("reader-fonts.css", "text/css; charset=utf-8"),
        ("manifest.webmanifest", "application/manifest+json"),
        ("icon-192.png", "image/png"),
        ("icon-512.png", "image/png"),
    ];
    for &(name, ct) in expected {
        let res = get(&app, &format!("/static/{name}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{name}");
        assert_eq!(
            res.headers()[header::CONTENT_TYPE].to_str().unwrap(),
            ct,
            "{name}"
        );
        let bytes = body_bytes(res).await;
        if name.ends_with(".woff2") {
            assert_eq!(&bytes[..4], &b"wOF2"[..], "{name}");
        }
        if name.ends_with(".png") {
            assert_eq!(&bytes[..4], b"\x89PNG", "{name}");
        }
        if name == "manifest.webmanifest" {
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(v["icons"].as_array().unwrap().len(), 2);
        }
        if name == "icons.svg" {
            let text = String::from_utf8(bytes).unwrap();
            assert!(text.contains("<symbol id=\"menu_book\""));
        }
    }
}

const READER_FONTS: [&str; 30] = [
    "atkinson-hyperlegible-latin-400-italic.woff2",
    "atkinson-hyperlegible-latin-400-normal.woff2",
    "atkinson-hyperlegible-latin-700-italic.woff2",
    "atkinson-hyperlegible-latin-700-normal.woff2",
    "atkinson-hyperlegible-latin-ext-400-italic.woff2",
    "atkinson-hyperlegible-latin-ext-400-normal.woff2",
    "atkinson-hyperlegible-latin-ext-700-italic.woff2",
    "atkinson-hyperlegible-latin-ext-700-normal.woff2",
    "eb-garamond-latin-ext-wght-italic.woff2",
    "eb-garamond-latin-ext-wght-normal.woff2",
    "eb-garamond-latin-wght-italic.woff2",
    "eb-garamond-latin-wght-normal.woff2",
    "inter-latin-ext-wght-italic.woff2",
    "inter-latin-ext-wght-normal.woff2",
    "inter-latin-wght-italic.woff2",
    "inter-latin-wght-normal.woff2",
    "lexend-latin-ext-wght-normal.woff2",
    "lexend-latin-wght-normal.woff2",
    "literata-latin-ext-wght-italic.woff2",
    "literata-latin-ext-wght-normal.woff2",
    "literata-latin-wght-italic.woff2",
    "literata-latin-wght-normal.woff2",
    "merriweather-latin-ext-wght-italic.woff2",
    "merriweather-latin-ext-wght-normal.woff2",
    "merriweather-latin-wght-italic.woff2",
    "merriweather-latin-wght-normal.woff2",
    "source-serif-4-latin-ext-wght-italic.woff2",
    "source-serif-4-latin-ext-wght-normal.woff2",
    "source-serif-4-latin-wght-italic.woff2",
    "source-serif-4-latin-wght-normal.woff2",
];

#[tokio::test]
async fn reader_fonts_are_served_as_woff2() {
    let (app, _dirs) = test_app().await;
    for name in READER_FONTS {
        let res = get(&app, &format!("/static/fonts/{name}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{name}");
        let ct = res.headers()[header::CONTENT_TYPE].to_str().unwrap();
        assert_eq!(ct, "font/woff2", "{name}");
        let bytes = body_bytes(res).await;
        assert_eq!(&bytes[..4], &b"wOF2"[..], "{name}");
    }
}

#[tokio::test]
async fn unknown_and_internal_static_names_are_404() {
    let (app, _dirs) = test_app().await;
    for name in [
        "bilinmeyen.js",
        "theme.test.mjs",
        "fonts/LICENSE",
        "fonts/LICENSE-reader.txt",
    ] {
        let res = get(&app, &format!("/static/{name}")).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{name}");
    }
}

#[tokio::test]
async fn index_html_script_and_style_rules() {
    let (app, _dirs) = test_app().await;
    let res = get(&app, "/").await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(body_bytes(res).await).unwrap();

    // theme.js, diğer tüm betiklerden önce gelmeli
    let theme = html.find("/static/theme.js").expect("theme.js yok");
    for other in [
        "tracker.js",
        "calc.js",
        "reader-ui.js",
        "reader-comic.js",
        "reader-text.js",
        "stats.js",
        "app.js",
    ] {
        let pos = html.find(&format!("/static/{other}")).expect(other);
        assert!(theme < pos, "theme.js, {other} betiğinden önce olmalı");
    }

    // her <script> harici (src="/static/...") ve defer/async içermemeli
    let mut rest = html.as_str();
    let mut count = 0;
    while let Some(i) = rest.find("<script") {
        let after = &rest[i..];
        let end = after.find('>').expect("kapanmamış <script");
        let tag = &after[..=end];
        assert!(
            tag.contains(" src=\"/static/"),
            "satır içi veya harici olmayan betik: {tag}"
        );
        assert!(!tag.contains("defer") && !tag.contains("async"), "{tag}");
        count += 1;
        rest = &after[end + 1..];
    }
    assert_eq!(count, 8);

    // calc.js ve reader-ui.js, reader-comic.js'ten önce
    let comic = html.find("/static/reader-comic.js").unwrap();
    for early in ["calc.js", "reader-ui.js"] {
        assert!(html.find(&format!("/static/{early}")).unwrap() < comic);
    }
    for needle in [
        "rel=\"manifest\"",
        "/static/reader-fonts.css",
        "apple-touch-icon",
    ] {
        assert!(html.contains(needle), "{needle}");
    }

    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "CDN bağlantısı var"
    );
    assert!(!html.contains("style="), "style= özniteliği var");
}
