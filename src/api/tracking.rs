use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    auth::{unix_now, AuthUser},
    error::{AppError, JsonBody},
};

// 0.99 değil: istemci sonda tam 1.0 yollar, yoksa 300 sayfalık kitap 3 sayfa erken biter
const FINISHED_PERCENT: f64 = 0.9999;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/books/{id}/progress",
            get(get_progress).put(put_progress),
        )
        .route("/api/books/{id}/status", put(put_status))
        .route("/api/reading/ping", post(reading_ping))
        .route("/api/history", get(get_history))
}

pub fn parse_local_date(s: &str) -> Result<(i32, u32, u32), AppError> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return Err(AppError::BadRequest("Geçersiz tarih biçimi."));
    }
    for &idx in &[0usize, 1, 2, 3, 5, 6, 8, 9] {
        if !b[idx].is_ascii_digit() {
            return Err(AppError::BadRequest("Geçersiz tarih biçimi."));
        }
    }

    let year: i32 = s[0..4]
        .parse()
        .map_err(|_| AppError::BadRequest("Geçersiz yıl."))?;
    let month: u32 = s[5..7]
        .parse()
        .map_err(|_| AppError::BadRequest("Geçersiz ay."))?;
    let day: u32 = s[8..10]
        .parse()
        .map_err(|_| AppError::BadRequest("Geçersiz gün."))?;

    if !(1900..=2200).contains(&year) || !(1..=12).contains(&month) {
        return Err(AppError::BadRequest("Geçersiz takvim günü."));
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap => 29,
        2 => 28,
        _ => return Err(AppError::BadRequest("Geçersiz ay.")),
    };

    if !(1..=max_day).contains(&day) {
        return Err(AppError::BadRequest("Geçersiz takvim günü."));
    }

    Ok((year, month, day))
}

#[derive(Serialize)]
struct ProgressRes {
    locator: String,
    percent: f64,
    status: String,
    updated_at: i64,
}

async fn get_progress(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ProgressRes>, AppError> {
    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM books WHERE id = ?1 AND missing = 0",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::NotFound);
            }

            let row: Option<ProgressRes> = conn
                .query_row(
                    "SELECT locator, percent, status, updated_at
                     FROM progress
                     WHERE user_id = ?1 AND book_id = ?2",
                    rusqlite::params![user_id, id],
                    |r| {
                        Ok(ProgressRes {
                            locator: r.get(0)?,
                            percent: r.get(1)?,
                            status: r.get(2)?,
                            updated_at: r.get(3)?,
                        })
                    },
                )
                .optional()?;

            Ok(row.unwrap_or_else(|| ProgressRes {
                locator: String::new(),
                percent: 0.0,
                status: "unread".to_string(),
                updated_at: 0,
            }))
        })
        .await?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct PutProgressReq {
    locator: String,
    percent: f64,
    local_date: String,
    #[serde(default)]
    client_ts: Option<i64>,
}

#[derive(Serialize)]
struct PutProgressRes {
    status: String,
    percent: f64,
    applied: bool,
    updated_at: i64,
}

async fn put_progress(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(req): JsonBody<PutProgressReq>,
) -> Result<Json<PutProgressRes>, AppError> {
    if !req.percent.is_finite() || !(0.0..=1.0).contains(&req.percent) {
        return Err(AppError::BadRequest("Geçersiz ilerleme yüzdesi."));
    }
    if req.locator.len() > 200 {
        return Err(AppError::BadRequest("Konum bilgisi çok uzun."));
    }
    parse_local_date(&req.local_date)?;
    if matches!(req.client_ts, Some(t) if t < 1) {
        return Err(AppError::BadRequest("Geçersiz client_ts."));
    }

    let user_id = user.id;
    let now = unix_now();
    let percent = req.percent;
    let locator = req.locator;
    let local_date = req.local_date;
    let client_ts = req.client_ts;

    let (status, percent, applied, updated_at) = state
        .db
        .run(move |conn| {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM books WHERE id = ?1 AND missing = 0",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::NotFound);
            }

            let prev: Option<(String, Option<String>, i64, f64)> = conn
                .query_row(
                    "SELECT status, finished_date, updated_at, percent
                     FROM progress
                     WHERE user_id = ?1 AND book_id = ?2",
                    rusqlite::params![user_id, id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;

            // istemci damgası gelecekten gelemez; yoksa sunucu saati
            let ts = client_ts.map_or(now, |t| t.min(now));
            if let Some((ref st, _, prev_ts, prev_pct)) = prev {
                if prev_ts > ts {
                    return Ok((st.clone(), prev_pct, false, prev_ts));
                }
            }

            let (new_status, new_finished_date) = if percent >= FINISHED_PERCENT {
                let fin_date = match prev {
                    Some((ref st, Some(ref d), _, _)) if st == "finished" => Some(d.clone()),
                    _ => Some(local_date),
                };
                ("finished".to_string(), fin_date)
            } else if let Some((ref st, ref fin_date, _, _)) = prev {
                if st == "finished" {
                    ("finished".to_string(), fin_date.clone())
                } else {
                    ("reading".to_string(), None)
                }
            } else {
                ("reading".to_string(), None)
            };

            conn.execute(
                "INSERT INTO progress(
                    user_id, book_id, locator, percent, status, finished_date, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(user_id, book_id) DO UPDATE SET
                    locator = excluded.locator,
                    percent = excluded.percent,
                    status = excluded.status,
                    finished_date = excluded.finished_date,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    user_id,
                    id,
                    locator,
                    percent,
                    new_status,
                    new_finished_date,
                    ts
                ],
            )?;

            Ok((new_status, percent, true, ts))
        })
        .await?;

    Ok(Json(PutProgressRes {
        status,
        percent,
        applied,
        updated_at,
    }))
}

#[derive(Deserialize)]
struct PutStatusReq {
    status: String,
    local_date: String,
    #[serde(default)]
    client_ts: Option<i64>,
}

#[derive(Serialize)]
struct PutStatusRes {
    status: String,
    applied: bool,
    updated_at: i64,
}

async fn put_status(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(req): JsonBody<PutStatusReq>,
) -> Result<Json<PutStatusRes>, AppError> {
    if !matches!(
        req.status.as_str(),
        "unread" | "reading" | "finished" | "dropped"
    ) {
        return Err(AppError::BadRequest("Geçersiz okuma durumu."));
    }
    parse_local_date(&req.local_date)?;
    if matches!(req.client_ts, Some(t) if t < 1) {
        return Err(AppError::BadRequest("Geçersiz client_ts."));
    }

    let user_id = user.id;
    let now = unix_now();
    let status = req.status;
    let local_date = req.local_date;
    let client_ts = req.client_ts;

    let (status, applied, updated_at) = state
        .db
        .run(move |conn| {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM books WHERE id = ?1 AND missing = 0",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::NotFound);
            }

            let prev: Option<(String, Option<String>, i64)> = conn
                .query_row(
                    "SELECT status, finished_date, updated_at
                     FROM progress
                     WHERE user_id = ?1 AND book_id = ?2",
                    rusqlite::params![user_id, id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;

            // istemci damgası gelecekten gelemez; yoksa sunucu saati
            let ts = client_ts.map_or(now, |t| t.min(now));
            if let Some((ref st, _, prev_ts)) = prev {
                if prev_ts > ts {
                    return Ok((st.clone(), false, prev_ts));
                }
            }

            let finished_date = if status == "finished" {
                match prev {
                    Some((ref st, Some(ref d), _)) if st == "finished" => Some(d.clone()),
                    _ => Some(local_date),
                }
            } else {
                None
            };

            conn.execute(
                "INSERT INTO progress(
                    user_id, book_id, locator, percent, status, finished_date, updated_at
                 ) VALUES (?1, ?2, '', 0.0, ?3, ?4, ?5)
                 ON CONFLICT(user_id, book_id) DO UPDATE SET
                    status = excluded.status,
                    finished_date = excluded.finished_date,
                    updated_at = excluded.updated_at",
                rusqlite::params![user_id, id, status, finished_date, ts],
            )?;
            Ok((status, true, ts))
        })
        .await?;

    Ok(Json(PutStatusRes {
        status,
        applied,
        updated_at,
    }))
}

#[derive(Deserialize)]
struct PingReq {
    client_id: String,
    book_id: i64,
    active_seconds: i64,
    pages_read: i64,
    start_percent: f64,
    end_percent: f64,
    local_date: String,
}

fn valid_client_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

async fn reading_ping(
    user: AuthUser,
    State(state): State<AppState>,
    JsonBody(req): JsonBody<PingReq>,
) -> Result<StatusCode, AppError> {
    if !valid_client_id(&req.client_id) {
        return Err(AppError::BadRequest("Geçersiz client_id."));
    }
    if !(0..=86_400).contains(&req.active_seconds) {
        return Err(AppError::BadRequest("Geçersiz active_seconds."));
    }
    if !(0..=100_000).contains(&req.pages_read) {
        return Err(AppError::BadRequest("Geçersiz pages_read."));
    }
    if !req.start_percent.is_finite()
        || !(0.0..=1.0).contains(&req.start_percent)
        || !req.end_percent.is_finite()
        || !(0.0..=1.0).contains(&req.end_percent)
    {
        return Err(AppError::BadRequest("Geçersiz yüzde değeri."));
    }
    parse_local_date(&req.local_date)?;

    let user_id = user.id;
    let now = unix_now();

    state
        .db
        .run(move |conn| {
            let book_exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM books WHERE id = ?1",
                    rusqlite::params![req.book_id],
                    |r| r.get(0),
                )
                .optional()?;
            if book_exists.is_none() {
                return Err(AppError::NotFound);
            }

            let existing: Option<(i64, i64, i64, i64, i64)> = conn
                .query_row(
                    "SELECT id, book_id, started_at, active_seconds, pages_read
                     FROM reading_sessions
                     WHERE user_id = ?1 AND client_id = ?2",
                    rusqlite::params![user_id, req.client_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;

            if let Some((row_id, existing_book_id, started_at, old_active, old_pages)) = existing {
                if existing_book_id != req.book_id {
                    return Err(AppError::Conflict("Bu oturum başka bir kitaba ait."));
                }
                let upper_cap = (now - started_at).max(0) + 5;
                let new_active = old_active.max(req.active_seconds).min(upper_cap);
                let new_pages = old_pages.max(req.pages_read);
                conn.execute(
                    "UPDATE reading_sessions
                     SET active_seconds = ?1, pages_read = ?2, end_percent = ?3, ended_at = ?4
                     WHERE id = ?5",
                    rusqlite::params![new_active, new_pages, req.end_percent, now, row_id],
                )?;
            } else {
                if req.active_seconds == 0 {
                    return Ok(());
                }
                let started_at = (now - req.active_seconds).max(0);
                conn.execute(
                    "INSERT INTO reading_sessions(
                        user_id, book_id, client_id, started_at, ended_at,
                        active_seconds, pages_read, start_percent, end_percent, local_date
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        user_id,
                        req.book_id,
                        req.client_id,
                        started_at,
                        now,
                        req.active_seconds,
                        req.pages_read,
                        req.start_percent,
                        req.end_percent,
                        req.local_date
                    ],
                )?;
            }

            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    date: Option<String>,
}

#[derive(Serialize)]
struct HistoryItem {
    id: i64,
    book_id: i64,
    book_title: String,
    series_name: String,
    book_missing: bool,
    started_at: i64,
    ended_at: i64,
    active_seconds: i64,
    pages_read: i64,
    start_percent: f64,
    end_percent: f64,
    local_date: String,
}

#[derive(Serialize)]
struct HistoryRes {
    total: i64,
    items: Vec<HistoryItem>,
}

async fn get_history(
    user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryRes>, AppError> {
    if let Some(d) = &query.date {
        parse_local_date(d)?;
    }
    let date = query.date;
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err(AppError::BadRequest("Geçersiz sayfalama parametresi."));
    }

    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM reading_sessions
                 WHERE user_id = ?1 AND (?2 IS NULL OR local_date = ?2)",
                rusqlite::params![user_id, date],
                |r| r.get(0),
            )?;

            let mut stmt = conn.prepare(
                "SELECT
                    rs.id,
                    rs.book_id,
                    b.title,
                    s.name,
                    b.missing,
                    rs.started_at,
                    rs.ended_at,
                    rs.active_seconds,
                    rs.pages_read,
                    rs.start_percent,
                    rs.end_percent,
                    rs.local_date
                 FROM reading_sessions rs
                 JOIN books b ON b.id = rs.book_id
                 JOIN series s ON s.id = b.series_id
                 WHERE rs.user_id = ?1 AND (?4 IS NULL OR rs.local_date = ?4)
                 ORDER BY rs.started_at DESC, rs.id DESC
                 LIMIT ?2 OFFSET ?3",
            )?;

            let items = stmt
                .query_map(rusqlite::params![user_id, limit, offset, date], |r| {
                    Ok(HistoryItem {
                        id: r.get(0)?,
                        book_id: r.get(1)?,
                        book_title: r.get(2)?,
                        series_name: r.get(3)?,
                        book_missing: r.get::<_, i64>(4)? != 0,
                        started_at: r.get(5)?,
                        ended_at: r.get(6)?,
                        active_seconds: r.get(7)?,
                        pages_read: r.get(8)?,
                        start_percent: r.get(9)?,
                        end_percent: r.get(10)?,
                        local_date: r.get(11)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(HistoryRes { total, items })
        })
        .await?;

    Ok(Json(res))
}
