mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::{body_json, login_as_admin, make_cbz, png, test_app};
use readcrab::{app::APP_CSP, safepath::resolve_book_path, scan::scan_once};
use tower::ServiceExt;

#[test]
fn resolve_book_path_rejects_dotdot_abs_symlink() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    let real_file = root.path().join("seri/kitap.cbz");
    std::fs::create_dir_all(real_file.parent().unwrap()).unwrap();
    std::fs::write(&real_file, b"ok").unwrap();

    let resolved = resolve_book_path(root.path(), "seri/kitap.cbz").unwrap();
    assert_eq!(resolved, real_file.canonicalize().unwrap());

    assert!(resolve_book_path(root.path(), "../outside.cbz").is_err());
    assert!(resolve_book_path(root.path(), "seri/../../outside.cbz").is_err());
    assert!(resolve_book_path(root.path(), "/etc/passwd").is_err());
    assert!(resolve_book_path(root.path(), "").is_err());

    let out_file = outside.path().join("secret.cbz");
    std::fs::write(&out_file, b"secret").unwrap();
    let sym = root.path().join("seri/link.cbz");
    std::os::unix::fs::symlink(&out_file, &sym).unwrap();

    assert!(resolve_book_path(root.path(), "seri/link.cbz").is_err());
}

#[tokio::test]
async fn sql_injection_in_search_is_inert() {
    let (app, dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;

    make_cbz(
        dirs.books_dir.path(),
        "NormalSeri/kitap.cbz",
        &[("p1.png", png(20, 20, [10, 20, 30]))],
    );
    assert!(scan_once(&dirs.state).await);

    let payloads = [
        "%27%20OR%201%3D1%20--",
        "%27%3B%20DROP%20TABLE%20books%3B%20--",
        "%25",
        "_",
    ];
    for q in payloads {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/series?q={q}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "q={q}");
        let json = body_json(res).await;
        assert_eq!(json["total"], 0, "q={q}");
    }

    let check_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/series")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let check_json = body_json(check_res).await;
    assert_eq!(check_json["total"], 1);
}

#[tokio::test]
async fn security_headers_present() {
    let (app, _dirs) = test_app().await;

    for uri in ["/", "/health", "/api/setup-status"] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            res.headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap()
                .to_str()
                .unwrap(),
            "nosniff"
        );
        assert_eq!(
            res.headers()
                .get(header::REFERRER_POLICY)
                .unwrap()
                .to_str()
                .unwrap(),
            "no-referrer"
        );
        assert_eq!(
            res.headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap()
                .to_str()
                .unwrap(),
            APP_CSP
        );
    }
}

async fn cover_status(
    app: &axum::Router,
    cookie: &str,
    uri: &str,
    ct: &str,
    origin: Option<&str>,
) -> StatusCode {
    let body = png(4, 4, [1, 2, 3]);
    let (st, _) = common::raw(app, "PUT", uri, cookie, ct, origin, body).await;
    st
}

#[tokio::test]
async fn cover_put_content_type_and_origin_guard() {
    let (app, _dirs) = test_app().await;
    let cookie = login_as_admin(&app).await;
    let cover = "/api/books/1/cover";
    let unsupported = StatusCode::UNSUPPORTED_MEDIA_TYPE;

    for ct in ["text/plain", "image/gif", "application/json"] {
        let st = cover_status(&app, &cookie, cover, ct, None).await;
        assert_eq!(st, unsupported, "{ct}");
    }
    let status_uri = "/api/books/1/status";
    let st = cover_status(&app, &cookie, status_uri, "image/png", None).await;
    assert_eq!(st, unsupported);
    let evil = Some("http://evil.example");
    let st = cover_status(&app, &cookie, cover, "image/png", evil).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}
