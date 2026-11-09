use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    auth::{unix_now, AuthUser},
    error::{AppError, JsonBody},
};

const MAX_PER_BOOK: i64 = 100;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/books/{id}/bookmarks",
            get(list_bookmarks).post(add_bookmark),
        )
        .route("/api/bookmarks/{id}", delete(delete_bookmark))
}

#[derive(Serialize)]
struct Bookmark {
    id: i64,
    locator: String,
    label: String,
    created_at: i64,
}

#[derive(Serialize)]
struct ListRes {
    items: Vec<Bookmark>,
}

#[derive(Deserialize)]
struct AddReq {
    locator: String,
    label: String,
}

fn ensure_book(conn: &Connection, id: i64) -> Result<(), AppError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM books WHERE id = ?1 AND missing = 0",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    found.map(|_| ()).ok_or(AppError::NotFound)
}

async fn list_bookmarks(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ListRes>, AppError> {
    let user_id = user.id;
    let items = state
        .db
        .run(move |conn| {
            ensure_book(conn, id)?;
            let mut stmt = conn.prepare(
                "SELECT id, locator, label, created_at FROM bookmarks
                 WHERE user_id = ?1 AND book_id = ?2
                 ORDER BY created_at DESC, id DESC",
            )?;
            let items = stmt
                .query_map([user_id, id], |r| {
                    Ok(Bookmark {
                        id: r.get(0)?,
                        locator: r.get(1)?,
                        label: r.get(2)?,
                        created_at: r.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
        .await?;
    Ok(Json(ListRes { items }))
}

async fn add_bookmark(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(req): JsonBody<AddReq>,
) -> Result<(StatusCode, Json<Bookmark>), AppError> {
    let label = req.label.trim().to_string();
    if req.locator.is_empty()
        || req.locator.len() > 200
        || label.is_empty()
        || label.chars().count() > 120
    {
        return Err(AppError::BadRequest("Geçersiz yer imi."));
    }
    let user_id = user.id;
    let locator = req.locator;
    let bookmark = state
        .db
        .run(move |conn| {
            ensure_book(conn, id)?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM bookmarks WHERE user_id = ?1 AND book_id = ?2",
                [user_id, id],
                |r| r.get(0),
            )?;
            if count >= MAX_PER_BOOK {
                return Err(AppError::Conflict("Yer imi sınırına ulaşıldı."));
            }
            let created_at = unix_now();
            conn.execute(
                "INSERT INTO bookmarks(user_id, book_id, locator, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![user_id, id, locator, label, created_at],
            )?;
            Ok(Bookmark {
                id: conn.last_insert_rowid(),
                locator,
                label,
                created_at,
            })
        })
        .await?;
    Ok((StatusCode::CREATED, Json(bookmark)))
}

async fn delete_bookmark(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let user_id = user.id;
    let n = state
        .db
        .run(move |conn| {
            Ok(conn.execute(
                "DELETE FROM bookmarks WHERE id = ?1 AND user_id = ?2",
                [id, user_id],
            )?)
        })
        .await?;
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
