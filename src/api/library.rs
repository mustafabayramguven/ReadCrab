use std::sync::atomic::Ordering;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, auth::AuthUser, error::AppError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/scan-status", get(scan_status))
        .route("/api/series", get(list_series))
        .route("/api/series/{id}", get(get_series))
        .route("/api/books/{id}", get(get_book))
        .route("/api/books/{id}/cover", get(get_book_cover))
}

#[derive(Serialize)]
struct ScanStatusRes {
    running: bool,
    done: u32,
    total: u32,
    last_finished_at: i64,
}

async fn scan_status(_user: AuthUser, State(state): State<AppState>) -> Json<ScanStatusRes> {
    Json(ScanStatusRes {
        running: state.scan.running.load(Ordering::SeqCst),
        done: state.scan.done.load(Ordering::SeqCst),
        total: state.scan.total.load(Ordering::SeqCst),
        last_finished_at: state.scan.last_finished_at.load(Ordering::SeqCst),
    })
}

#[derive(Deserialize)]
struct SeriesQuery {
    q: Option<String>,
    sort: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
struct SeriesListItem {
    id: i64,
    name: String,
    book_count: i64,
    read_count: i64,
    reading_count: i64,
    percent: f64,
    cover_book_id: Option<i64>,
    last_read_at: Option<i64>,
}

#[derive(Serialize)]
struct SeriesListRes {
    total: i64,
    items: Vec<SeriesListItem>,
}

fn like_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

async fn list_series(
    user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<SeriesQuery>,
) -> Result<Json<SeriesListRes>, AppError> {
    let q = query.q.unwrap_or_default().trim().to_string();
    let sort = query.sort.unwrap_or_else(|| "name".to_string());
    let status = query.status.unwrap_or_else(|| "all".to_string());
    let limit = query.limit.unwrap_or(60);
    let offset = query.offset.unwrap_or(0);

    if !(1..=200).contains(&limit) || offset < 0 {
        return Err(AppError::BadRequest("Geçersiz sayfalama parametresi."));
    }
    if !matches!(sort.as_str(), "name" | "added" | "recent") {
        return Err(AppError::BadRequest("Geçersiz sıralama parametresi."));
    }
    if !matches!(status.as_str(), "all" | "reading" | "finished" | "unread") {
        return Err(AppError::BadRequest("Geçersiz durum süzgeci."));
    }

    let order_clause = match sort.as_str() {
        "added" => "s.added_at DESC, s.id DESC",
        "recent" => {
            "CASE WHEN last_read_at IS NULL THEN 1 ELSE 0 END ASC,
             last_read_at DESC,
             s.name COLLATE NOCASE ASC,
             s.id ASC"
        }
        _ => "s.name COLLATE NOCASE ASC, s.id ASC",
    };

    let pattern = if q.is_empty() {
        None
    } else {
        Some(format!("%{}%", like_escape(&q)))
    };

    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let filter_sql = if pattern.is_some() {
                "AND (
                    s.name LIKE ?2 ESCAPE '\\'
                    OR EXISTS (
                        SELECT 1 FROM books b2
                        WHERE b2.series_id = s.id
                          AND b2.missing = 0
                          AND b2.title LIKE ?2 ESCAPE '\\'
                    )
                )"
            } else {
                ""
            };

            let sql = format!(
                "SELECT
                    s.id,
                    s.name,
                    COUNT(b.id) AS book_count,
                    SUM(CASE WHEN p.status = 'finished' THEN 1 ELSE 0 END) AS read_count,
                    SUM(CASE WHEN p.status = 'reading' THEN 1 ELSE 0 END) AS reading_count,
                    AVG(
                        CASE
                            WHEN p.status = 'finished' THEN 1.0
                            ELSE COALESCE(p.percent, 0.0)
                        END
                    ) AS avg_percent,
                    (
                        SELECT b3.id FROM books b3
                        WHERE b3.series_id = s.id AND b3.missing = 0 AND b3.has_cover = 1
                        ORDER BY b3.sort_index ASC, b3.id ASC
                        LIMIT 1
                    ) AS cover_book_id,
                    MAX(p.updated_at) AS last_read_at
                 FROM series s
                 JOIN books b ON b.series_id = s.id AND b.missing = 0
                 LEFT JOIN progress p ON p.book_id = b.id AND p.user_id = ?1
                 WHERE 1 = 1 {filter_sql}
                 GROUP BY s.id
                 ORDER BY {order_clause}"
            );

            let mut stmt = conn.prepare(&sql)?;
            let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<SeriesListItem> {
                Ok(SeriesListItem {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    book_count: r.get(2)?,
                    read_count: r.get(3)?,
                    reading_count: r.get(4)?,
                    percent: r.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                    cover_book_id: r.get(6)?,
                    last_read_at: r.get(7)?,
                })
            };

            let rows: Vec<SeriesListItem> = if let Some(ref pat) = pattern {
                stmt.query_map(rusqlite::params![user_id, pat], map_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            } else {
                stmt.query_map(rusqlite::params![user_id], map_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };

            let filtered: Vec<SeriesListItem> = rows
                .into_iter()
                .filter(|it| match status.as_str() {
                    "reading" => {
                        it.reading_count > 0 || (it.read_count > 0 && it.read_count < it.book_count)
                    }
                    "finished" => it.read_count == it.book_count,
                    "unread" => it.read_count == 0 && it.reading_count == 0,
                    _ => true,
                })
                .collect();

            let total = filtered.len() as i64;
            let items = filtered
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .collect();

            Ok(SeriesListRes { total, items })
        })
        .await?;

    Ok(Json(res))
}

#[derive(Serialize)]
struct SeriesBookItem {
    id: i64,
    title: String,
    format: String,
    page_count: i64,
    size: i64,
    error: Option<String>,
    status: String,
    percent: f64,
}

#[derive(Serialize)]
struct SeriesDetailRes {
    id: i64,
    name: String,
    books: Vec<SeriesBookItem>,
}

async fn get_series(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SeriesDetailRes>, AppError> {
    let user_id = user.id;
    let detail = state
        .db
        .run(move |conn| {
            let name: Option<String> = conn
                .query_row(
                    "SELECT name FROM series WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(name) = name else {
                return Err(AppError::NotFound);
            };

            let mut stmt = conn.prepare(
                "SELECT
                    b.id,
                    b.title,
                    b.format,
                    b.page_count,
                    b.size,
                    b.error,
                    COALESCE(p.status, 'unread'),
                    COALESCE(p.percent, 0.0)
                 FROM books b
                 LEFT JOIN progress p ON p.book_id = b.id AND p.user_id = ?1
                 WHERE b.series_id = ?2 AND b.missing = 0
                 ORDER BY b.sort_index ASC, b.id ASC",
            )?;
            let books = stmt
                .query_map(rusqlite::params![user_id, id], |r| {
                    Ok(SeriesBookItem {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        format: r.get(2)?,
                        page_count: r.get(3)?,
                        size: r.get(4)?,
                        error: r.get(5)?,
                        status: r.get(6)?,
                        percent: r.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            if books.is_empty() {
                return Err(AppError::NotFound);
            }

            Ok(SeriesDetailRes { id, name, books })
        })
        .await?;

    Ok(Json(detail))
}

#[derive(Serialize)]
struct BookDetailRes {
    id: i64,
    series_id: i64,
    series_name: String,
    title: String,
    format: String,
    page_count: i64,
    error: Option<String>,
    status: String,
    percent: f64,
    locator: String,
}

async fn get_book(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<BookDetailRes>, AppError> {
    let user_id = user.id;
    let book = state
        .db
        .run(move |conn| {
            let row = conn
                .query_row(
                    "SELECT
                        b.id,
                        b.series_id,
                        s.name,
                        b.title,
                        b.format,
                        b.page_count,
                        b.error,
                        COALESCE(p.status, 'unread'),
                        COALESCE(p.percent, 0.0),
                        COALESCE(p.locator, '')
                     FROM books b
                     JOIN series s ON s.id = b.series_id
                     LEFT JOIN progress p ON p.book_id = b.id AND p.user_id = ?1
                     WHERE b.id = ?2 AND b.missing = 0",
                    rusqlite::params![user_id, id],
                    |r| {
                        Ok(BookDetailRes {
                            id: r.get(0)?,
                            series_id: r.get(1)?,
                            series_name: r.get(2)?,
                            title: r.get(3)?,
                            format: r.get(4)?,
                            page_count: r.get(5)?,
                            error: r.get(6)?,
                            status: r.get(7)?,
                            percent: r.get(8)?,
                            locator: r.get(9)?,
                        })
                    },
                )
                .optional()?;
            row.ok_or(AppError::NotFound)
        })
        .await?;

    Ok(Json(book))
}

async fn get_book_cover(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let has_cover: Option<i64> = state
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT has_cover FROM books WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(AppError::from)
        })
        .await?;

    if has_cover != Some(1) {
        return Err(AppError::NotFound);
    }

    let cover_path = state.cfg.data_dir.join("covers").join(format!("{id}.jpg"));
    let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || std::fs::read(cover_path)).await else {
        return Err(AppError::NotFound);
    };

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=86400"),
            ),
        ],
        bytes,
    )
        .into_response())
}
