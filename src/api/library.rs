use std::{collections::HashSet, sync::atomic::Ordering};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, auth::AuthUser, error::AppError, formats::natural::natural_cmp};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/scan-status", get(scan_status))
        .route("/api/continue", get(continue_reading))
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
    cover_v: Option<i64>,
    last_read_at: Option<i64>,
}

#[derive(Serialize)]
struct SeriesListRes {
    total: i64,
    items: Vec<SeriesListItem>,
}

pub fn fold_tr(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'İ' | 'I' | 'ı' => 'i',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase()
}

struct SeriesRow {
    item: SeriesListItem,
    added_at: i64,
    folded_name: String,
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

    let q_folded = if q.is_empty() {
        None
    } else {
        Some(fold_tr(&q))
    };

    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let matching_book_series: HashSet<i64> = if let Some(ref qf) = q_folded {
                let mut bstmt =
                    conn.prepare("SELECT series_id, title FROM books WHERE missing = 0")?;
                let pairs =
                    bstmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                let mut set = HashSet::new();
                for pair in pairs {
                    let (sid, title) = pair?;
                    if fold_tr(&title).contains(qf) {
                        set.insert(sid);
                    }
                }
                set
            } else {
                HashSet::new()
            };

            let mut stmt = conn.prepare(
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
                        ORDER BY b3.custom_cover DESC, b3.sort_index ASC, b3.id ASC
                        LIMIT 1
                    ) AS cover_book_id,
                    MAX(p.updated_at) AS last_read_at,
                    s.added_at,
                    (
                        SELECT b3.cover_version FROM books b3
                        WHERE b3.series_id = s.id AND b3.missing = 0 AND b3.has_cover = 1
                        ORDER BY b3.custom_cover DESC, b3.sort_index ASC, b3.id ASC
                        LIMIT 1
                    ) AS cover_v
                 FROM series s
                 JOIN books b ON b.series_id = s.id AND b.missing = 0
                 LEFT JOIN progress p ON p.book_id = b.id AND p.user_id = ?1
                 GROUP BY s.id",
            )?;

            let rows = stmt
                .query_map(rusqlite::params![user_id], |r| {
                    let name: String = r.get(1)?;
                    let folded_name = fold_tr(&name);
                    Ok(SeriesRow {
                        item: SeriesListItem {
                            id: r.get(0)?,
                            name,
                            book_count: r.get(2)?,
                            read_count: r.get(3)?,
                            reading_count: r.get(4)?,
                            percent: r.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                            cover_book_id: r.get(6)?,
                            cover_v: r.get(9)?,
                            last_read_at: r.get(7)?,
                        },
                        added_at: r.get(8)?,
                        folded_name,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut filtered: Vec<SeriesRow> = rows
                .into_iter()
                .filter(|row| {
                    if let Some(ref qf) = q_folded {
                        if !row.folded_name.contains(qf)
                            && !matching_book_series.contains(&row.item.id)
                        {
                            return false;
                        }
                    }
                    let it = &row.item;
                    match status.as_str() {
                        "reading" => {
                            it.reading_count > 0
                                || (it.read_count > 0 && it.read_count < it.book_count)
                        }
                        "finished" => it.read_count == it.book_count,
                        "unread" => it.read_count == 0 && it.reading_count == 0,
                        _ => true,
                    }
                })
                .collect();

            match sort.as_str() {
                "added" => {
                    filtered.sort_by(|a, b| {
                        b.added_at
                            .cmp(&a.added_at)
                            .then_with(|| b.item.id.cmp(&a.item.id))
                    });
                }
                "recent" => {
                    filtered.sort_by(|a, b| {
                        a.item
                            .last_read_at
                            .is_none()
                            .cmp(&b.item.last_read_at.is_none())
                            .then_with(|| b.item.last_read_at.cmp(&a.item.last_read_at))
                            .then_with(|| natural_cmp(&a.folded_name, &b.folded_name))
                            .then_with(|| a.item.id.cmp(&b.item.id))
                    });
                }
                _ => {
                    filtered.sort_by(|a, b| {
                        natural_cmp(&a.folded_name, &b.folded_name)
                            .then_with(|| a.item.id.cmp(&b.item.id))
                    });
                }
            }

            let total = filtered.len() as i64;
            let items = filtered
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|r| r.item)
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
    cover_v: i64,
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
                    CASE WHEN p.status = 'finished' THEN 1.0 ELSE COALESCE(p.percent, 0.0) END,
                    b.cover_version
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
                        cover_v: r.get(8)?,
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
    cover_version: i64,
    next_book_id: Option<i64>,
    next_book_title: Option<String>,
    prev_book_id: Option<i64>,
    pace_seconds: Option<i64>,
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
                        CASE WHEN p.status = 'finished' THEN 1.0 ELSE COALESCE(p.percent, 0.0) END,
                        COALESCE(p.locator, ''),
                        b.cover_version,
                        b.sort_index
                     FROM books b
                     JOIN series s ON s.id = b.series_id
                     LEFT JOIN progress p ON p.book_id = b.id AND p.user_id = ?1
                     WHERE b.id = ?2 AND b.missing = 0",
                    rusqlite::params![user_id, id],
                    |r| {
                        Ok((BookDetailRes {
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
                            cover_version: r.get(10)?,
                            next_book_id: None,
                            next_book_title: None,
                            prev_book_id: None,
                            pace_seconds: None,
                        }, r.get::<_, i64>(11)?))
                    },
                )
                .optional()?;
            let Some((mut book, sort_index)) = row else {
                return Err(AppError::NotFound);
            };
            let neighbor = |cmp: &str, dir: &str| -> rusqlite::Result<Option<(i64, String)>> {
                conn.query_row(
                    &format!(
                        "SELECT id, title FROM books
                         WHERE series_id = ?1 AND missing = 0 AND error IS NULL
                           AND (sort_index {cmp} ?2 OR (sort_index = ?2 AND id {cmp} ?3))
                         ORDER BY sort_index {dir}, id {dir} LIMIT 1"
                    ),
                    rusqlite::params![book.series_id, sort_index, id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
            };
            if let Some((nid, title)) = neighbor(">", "ASC")? {
                book.next_book_id = Some(nid);
                book.next_book_title = Some(title);
            }
            book.prev_book_id = neighbor("<", "DESC")?.map(|(pid, _)| pid);
            let (sa, sd): (Option<i64>, Option<f64>) = conn.query_row(
                "SELECT SUM(active_seconds), SUM(end_percent - start_percent)
                 FROM reading_sessions
                 WHERE user_id = ?1 AND book_id = ?2
                   AND end_percent > start_percent AND active_seconds >= 30",
                rusqlite::params![user_id, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            // shortcut: ileri sarma ve geri okuma ortalamayı bozar, gerekirse son N oturum
            if let (Some(sa), Some(sd)) = (sa, sd) {
                if sa >= 300 && sd >= 0.02 {
                    book.pace_seconds = Some((sa as f64 / sd).round() as i64);
                }
            }
            Ok(book)
        })
        .await?;

    Ok(Json(book))
}

#[derive(Serialize)]
struct ContinueItem {
    book_id: i64,
    title: String,
    series_id: i64,
    series_name: String,
    format: String,
    percent: f64,
    cover_v: i64,
}

async fn continue_reading(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = user.id;
    let items = state
        .db
        .run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT b.id, b.title, b.series_id, s.name, b.format, p.percent, b.cover_version
                 FROM progress p
                 JOIN books b ON b.id = p.book_id
                 JOIN series s ON s.id = b.series_id
                 WHERE p.user_id = ?1 AND p.status = 'reading' AND b.missing = 0 AND b.error IS NULL
                 ORDER BY p.updated_at DESC, b.id DESC
                 LIMIT 20",
            )?;
            let rows = stmt
                .query_map([user_id], |r| {
                    Ok(ContinueItem {
                        book_id: r.get(0)?,
                        title: r.get(1)?,
                        series_id: r.get(2)?,
                        series_name: r.get(3)?,
                        format: r.get(4)?,
                        percent: r.get(5)?,
                        cover_v: r.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(serde_json::json!({ "items": items })))
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
