use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    app::{AppState, MobiCache},
    auth::AuthUser,
    error::AppError,
    formats::{
        comic::{comic_pages_cached, comic_read, file_stamp},
        epub::{epub_asset, epub_chapter_html, epub_open},
        mobi::{mobi_open, MobiBook},
        sanitize::{frame_document, sanitize_chapter, SrcMode, FRAME_CSP},
        Format, FormatError, MAX_ENTRY_BYTES,
    },
    safepath::resolve_book_path,
};

// shortcut: tek kitaplık önbellek, iki kullanıcı dönüşümlü farklı MOBI okursa yeniden çözülür, gerekirse LRU
fn open_mobi_cached(cache: &MobiCache, path: &Path) -> Result<Arc<MobiBook>, FormatError> {
    let stamp = file_stamp(path)?;
    if let Some((cached_path, cached_stamp, book)) =
        cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
    {
        if cached_path == path && *cached_stamp == stamp {
            return Ok(book.clone());
        }
    }
    let book = Arc::new(mobi_open(path)?);
    *cache.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), stamp, book.clone()));
    Ok(book)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/books/{id}/pages", get(get_pages))
        .route("/api/books/{id}/pages/{n}", get(get_page))
        .route("/api/books/{id}/toc", get(get_toc))
        .route("/api/books/{id}/chapters/{n}", get(get_chapter))
        .route("/api/books/{id}/asset", get(get_asset))
}

struct BookMeta {
    full_path: PathBuf,
    format: Format,
    title: String,
    page_count: i64,
}

type BookRow = (String, String, String, i64, Option<String>, i64);

async fn load_book_meta(state: &AppState, id: i64) -> Result<BookMeta, AppError> {
    let row: Option<BookRow> = state
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT rel_path, format, title, page_count, error, missing
                 FROM books WHERE id = ?1",
                rusqlite::params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(AppError::from)
        })
        .await?;

    let Some((rel_path, fmt_str, title, page_count, error, missing)) = row else {
        return Err(AppError::NotFound);
    };
    if missing != 0 {
        return Err(AppError::NotFound);
    }
    match error.as_deref() {
        Some("drm") => return Err(AppError::Drm),
        Some(_) => return Err(AppError::Corrupt),
        None => {}
    }

    let format = Format::from_ext(&fmt_str).ok_or(AppError::Internal)?;
    let full_path = resolve_book_path(&state.cfg.books_dir, &rel_path)?;

    Ok(BookMeta {
        full_path,
        format,
        title,
        page_count,
    })
}

#[derive(Serialize)]
struct PagesRes {
    count: i64,
}

async fn get_pages(
    _user: AuthUser,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<PagesRes>, AppError> {
    let meta = load_book_meta(&state, id).await?;
    if !meta.format.is_comic() {
        return Err(AppError::BadRequest(
            "Bu uç nokta yalnız çizgi roman biçimleri içindir.",
        ));
    }
    Ok(Json(PagesRes {
        count: meta.page_count,
    }))
}

fn image_mime_from_name(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

async fn get_page(
    _user: AuthUser,
    State(state): State<AppState>,
    AxumPath((id, n)): AxumPath<(i64, usize)>,
) -> Result<Response, AppError> {
    let meta = load_book_meta(&state, id).await?;
    if !meta.format.is_comic() {
        return Err(AppError::BadRequest(
            "Bu uç nokta yalnız çizgi roman biçimleri içindir.",
        ));
    }

    let pages = comic_pages_cached(&meta.full_path, meta.format).await?;
    let Some(member) = pages.get(n) else {
        return Err(AppError::NotFound);
    };
    let mime = image_mime_from_name(member);
    let bytes = comic_read(&meta.full_path, meta.format, member, MAX_ENTRY_BYTES).await?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=3600"),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Serialize)]
struct TocChapter {
    index: usize,
    title: String,
}

#[derive(Serialize)]
struct TocRes {
    chapters: Vec<TocChapter>,
}

async fn get_toc(
    _user: AuthUser,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<TocRes>, AppError> {
    let meta = load_book_meta(&state, id).await?;
    if meta.format.is_comic() {
        return Err(AppError::BadRequest(
            "Bu uç nokta yalnız metin biçimleri içindir.",
        ));
    }

    let full_path = meta.full_path;
    let format = meta.format;
    let mobi_cache = state.mobi_cache.clone();
    let chapters = tokio::task::spawn_blocking(move || -> Result<Vec<TocChapter>, FormatError> {
        match format {
            Format::Epub => {
                let info = epub_open(&full_path)?;
                Ok(info
                    .toc
                    .into_iter()
                    .map(|(title, index)| TocChapter { index, title })
                    .collect())
            }
            Format::Mobi => {
                let book = open_mobi_cached(&mobi_cache, &full_path)?;
                Ok((0..book.chapters.len())
                    .map(|i| TocChapter {
                        index: i,
                        title: format!("Bölüm {}", i + 1),
                    })
                    .collect())
            }
            _ => Err(FormatError::Corrupt),
        }
    })
    .await
    .map_err(|_| AppError::Internal)??;

    Ok(Json(TocRes { chapters }))
}

async fn get_chapter(
    _user: AuthUser,
    State(state): State<AppState>,
    AxumPath((id, n)): AxumPath<(i64, usize)>,
) -> Result<Response, AppError> {
    let meta = load_book_meta(&state, id).await?;
    if meta.format.is_comic() {
        return Err(AppError::BadRequest(
            "Bu uç nokta yalnız metin biçimleri içindir.",
        ));
    }

    let full_path = meta.full_path;
    let format = meta.format;
    let book_title = meta.title;
    let mobi_cache = state.mobi_cache.clone();

    let doc_opt = tokio::task::spawn_blocking(move || -> Result<Option<String>, FormatError> {
        match format {
            Format::Epub => {
                let info = epub_open(&full_path)?;
                let Some(member) = info.spine.get(n).cloned() else {
                    return Ok(None);
                };
                let raw = epub_chapter_html(&full_path, &info, n)?;
                let chapter_dir = member
                    .rsplit_once('/')
                    .map(|(d, _)| d.to_string())
                    .unwrap_or_default();
                let body = sanitize_chapter(&raw, id, &chapter_dir, SrcMode::Epub);
                Ok(Some(frame_document(&body, &book_title)))
            }
            Format::Mobi => {
                let book = open_mobi_cached(&mobi_cache, &full_path)?;
                let Some(raw) = book.chapters.get(n) else {
                    return Ok(None);
                };
                let body = sanitize_chapter(raw, id, "", SrcMode::Mobi);
                Ok(Some(frame_document(&body, &book_title)))
            }
            _ => Err(FormatError::Corrupt),
        }
    })
    .await
    .map_err(|_| AppError::Internal)??;

    let Some(doc) = doc_opt else {
        return Err(AppError::NotFound);
    };

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(FRAME_CSP),
            ),
        ],
        doc,
    )
        .into_response())
}

#[derive(Deserialize)]
struct AssetQuery {
    p: Option<String>,
}

fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

async fn get_asset(
    _user: AuthUser,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Query(query): Query<AssetQuery>,
) -> Result<Response, AppError> {
    let meta = load_book_meta(&state, id).await?;
    if meta.format.is_comic() {
        return Err(AppError::BadRequest(
            "Bu uç nokta yalnız metin biçimleri içindir.",
        ));
    }

    let Some(p) = query.p.filter(|s| !s.is_empty()) else {
        return Err(AppError::NotFound);
    };

    let full_path = meta.full_path;
    let format = meta.format;
    let mobi_cache = state.mobi_cache.clone();

    let asset_opt =
        tokio::task::spawn_blocking(move || -> Result<Option<(Bytes, String)>, FormatError> {
            match format {
                Format::Epub => {
                    let info = epub_open(&full_path)?;
                    match epub_asset(&full_path, &info, &p) {
                        Ok((bytes, mime)) => Ok(Some((Bytes::from(bytes), mime))),
                        Err(FormatError::Drm) => Err(FormatError::Drm),
                        Err(_) => Ok(None),
                    }
                }
                Format::Mobi => {
                    let Ok(idx1) = p.parse::<usize>() else {
                        return Ok(None);
                    };
                    if idx1 == 0 {
                        return Ok(None);
                    }
                    let book = open_mobi_cached(&mobi_cache, &full_path)?;
                    let Some(bytes) = book.images.get(idx1 - 1).cloned() else {
                        return Ok(None);
                    };
                    let mime = sniff_image_mime(&bytes).to_string();
                    Ok(Some((bytes, mime)))
                }
                _ => Err(FormatError::Corrupt),
            }
        })
        .await
        .map_err(|_| AppError::Internal)??;

    let Some((bytes, mime)) = asset_opt else {
        return Err(AppError::NotFound);
    };

    let content_type =
        HeaderValue::from_str(&mime).unwrap_or_else(|_| HeaderValue::from_static("image/png"));

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("sandbox; default-src 'none'"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        bytes,
    )
        .into_response())
}
