use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::catch_panic::CatchPanicLayer;

use crate::{config::Config, db::Db, error::AppError};

pub const APP_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'self'";

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const TRACKER_JS: &str = include_str!("../web/tracker.js");
const READER_COMIC_JS: &str = include_str!("../web/reader-comic.js");
const READER_TEXT_JS: &str = include_str!("../web/reader-text.js");
const STATS_JS: &str = include_str!("../web/stats.js");
const THEME_JS: &str = include_str!("../web/theme.js");
const STYLE_CSS: &str = include_str!("../web/style.css");
const FRAME_CSS: &str = include_str!("../web/frame.css");
const FAVICON_SVG: &str = include_str!("../web/favicon.svg");
const ICONS_SVG: &str = include_str!("../web/icons.svg");
const FONT_LATIN: &[u8] = include_bytes!("../web/fonts/roboto-flex-latin.woff2");
const FONT_LATIN_EXT: &[u8] = include_bytes!("../web/fonts/roboto-flex-latin-ext.woff2");

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub cfg: Arc<Config>,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/", get(index_handler))
        .route("/static/{*ad}", get(static_handler))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn(security_headers))
        .layer(CatchPanicLayer::custom(|_| {
            AppError::Internal.into_response()
        }))
        .with_state(state)
}

async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    if res.status() == StatusCode::PAYLOAD_TOO_LARGE {
        let is_json = res
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("application/json"));
        if !is_json {
            res = AppError::PayloadTooLarge.into_response();
        }
    }
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(APP_CSP),
        );
    }
    res
}

async fn health_handler(State(state): State<AppState>) -> Response {
    let ok = state
        .db
        .run(|conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
                .map_err(AppError::from)
        })
        .await
        .is_ok();

    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = if ok { "ok" } else { "error" };
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn index_handler() -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        INDEX_HTML,
    )
        .into_response()
}

async fn static_handler(Path(ad): Path<String>) -> Result<Response, AppError> {
    let (bytes, content_type, cache_control): (&[u8], &'static str, &'static str) =
        match ad.as_str() {
            "app.js" => (
                APP_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "tracker.js" => (
                TRACKER_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "reader-comic.js" => (
                READER_COMIC_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "reader-text.js" => (
                READER_TEXT_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "stats.js" => (
                STATS_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "theme.js" => (
                THEME_JS.as_bytes(),
                "application/javascript; charset=utf-8",
                "no-cache",
            ),
            "style.css" => (STYLE_CSS.as_bytes(), "text/css; charset=utf-8", "no-cache"),
            "frame.css" => (FRAME_CSS.as_bytes(), "text/css; charset=utf-8", "no-cache"),
            "favicon.svg" => (
                FAVICON_SVG.as_bytes(),
                "image/svg+xml",
                "public, max-age=86400",
            ),
            "icons.svg" => (
                ICONS_SVG.as_bytes(),
                "image/svg+xml",
                "public, max-age=86400",
            ),
            "fonts/roboto-flex-latin.woff2" => (FONT_LATIN, "font/woff2", "public, max-age=86400"),
            "fonts/roboto-flex-latin-ext.woff2" => {
                (FONT_LATIN_EXT, "font/woff2", "public, max-age=86400")
            }
            _ => return Err(AppError::NotFound),
        };

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
        ],
        bytes,
    )
        .into_response())
}
