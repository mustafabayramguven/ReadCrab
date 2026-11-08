use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, put},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    auth::{hash_password, unix_now, validate_password, validate_username, AdminUser},
    error::{AppError, JsonBody},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(list_users).post(create_user))
        .route("/api/admin/users/{id}", delete(delete_user))
        .route("/api/admin/users/{id}/password", put(reset_password))
}

#[derive(Serialize)]
struct AdminUserItem {
    id: i64,
    username: String,
    is_admin: bool,
    created_at: i64,
}

async fn list_users(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminUserItem>>, AppError> {
    let items = state
        .db
        .run(|conn| {
            let sql = "SELECT id, username, is_admin, created_at FROM users ORDER BY id ASC";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(AdminUserItem {
                        id: r.get(0)?,
                        username: r.get(1)?,
                        is_admin: r.get::<_, i64>(2)? != 0,
                        created_at: r.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(items))
}

#[derive(Deserialize)]
struct CreateUserReq {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct CreatedUserRes {
    id: i64,
    username: String,
    is_admin: bool,
}

async fn create_user(
    _admin: AdminUser,
    State(state): State<AppState>,
    JsonBody(req): JsonBody<CreateUserReq>,
) -> Result<Response, AppError> {
    validate_username(&req.username)?;
    validate_password(&req.password)?;

    let pw_hash = hash_password(req.password).await?;
    let now = unix_now();
    let username = req.username.clone();

    let id = state
        .db
        .run(move |conn| {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM users WHERE username = ?1",
                    rusqlite::params![username],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_some() {
                return Err(AppError::Conflict("Bu kullanıcı adı zaten kullanılıyor."));
            }
            conn.execute(
                "INSERT INTO users(username, password_hash, is_admin, created_at)
                 VALUES (?1, ?2, 0, ?3)",
                rusqlite::params![username, pw_hash, now],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    AppError::Conflict("Bu kullanıcı adı zaten kullanılıyor.")
                }
                other => AppError::from(other),
            })?;
            Ok(conn.last_insert_rowid())
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedUserRes {
            id,
            username: req.username,
            is_admin: false,
        }),
    )
        .into_response())
}

async fn delete_user(
    AdminUser(admin): AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    if admin.id == id {
        return Err(AppError::Conflict("cannot_delete_self"));
    }

    state
        .db
        .run(move |conn| {
            let deleted = conn.execute("DELETE FROM users WHERE id = ?1", rusqlite::params![id])?;
            if deleted == 0 {
                return Err(AppError::NotFound);
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ResetPasswordReq {
    password: String,
}

async fn reset_password(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(req): JsonBody<ResetPasswordReq>,
) -> Result<StatusCode, AppError> {
    validate_password(&req.password)?;
    let pw_hash = hash_password(req.password).await?;

    state
        .db
        .run(move |conn| {
            let updated = conn.execute(
                "UPDATE users SET password_hash = ?1 WHERE id = ?2",
                rusqlite::params![pw_hash, id],
            )?;
            if updated == 0 {
                return Err(AppError::NotFound);
            }
            conn.execute(
                "DELETE FROM sessions WHERE user_id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
