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
    ];
    for &(name, ct) in expected {
        let res = get(&app, &format!("/static/{name}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{name}");
        assert_eq!(res.headers()[header::CONTENT_TYPE].to_str().unwrap(), ct, "{name}");
        let bytes = body_bytes(res).await;
        if name.ends_with(".woff2") {
            assert_eq!(&bytes[..4], &b"wOF2"[..], "{name}");
        }
        if name == "icons.svg" {
            let text = String::from_utf8(bytes).unwrap();
            assert!(text.contains("<symbol id=\"menu_book\""));
        }
    }
}

#[tokio::test]
async fn unknown_and_internal_static_names_are_404() {
    let (app, _dirs) = test_app().await;
    for name in ["bilinmeyen.js", "theme.test.mjs", "fonts/LICENSE"] {
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
    for other in ["tracker.js", "reader-comic.js", "reader-text.js", "stats.js", "app.js"] {
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
        assert!(tag.contains(" src=\"/static/"), "satır içi veya harici olmayan betik: {tag}");
        assert!(!tag.contains("defer") && !tag.contains("async"), "{tag}");
        count += 1;
        rest = &after[end + 1..];
    }
    assert_eq!(count, 6);

    assert!(!html.contains("http://") && !html.contains("https://"), "CDN bağlantısı var");
    assert!(!html.contains("style="), "style= özniteliği var");
}
