use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    auth::{
        build_session_cookie, extract_session_token, hash_password, new_session_token, token_hash,
        unix_now, validate_password, validate_username, verify_password, AuthUser, DUMMY_HASH,
        SESSION_TTL_SECS,
    },
    error::{AppError, JsonBody},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/setup-status", get(setup_status))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/me/password", put(change_password))
}

#[derive(Serialize)]
struct SetupStatusRes {
    needs_setup: bool,
}

async fn setup_status(State(state): State<AppState>) -> Result<Json<SetupStatusRes>, AppError> {
    let count: i64 = state
        .db
        .run(|conn| {
            conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .map_err(AppError::from)
        })
        .await?;
    Ok(Json(SetupStatusRes {
        needs_setup: count == 0,
    }))
}

#[derive(Deserialize)]
struct CredentialsReq {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct UserRes {
    id: i64,
    username: String,
    is_admin: bool,
}

async fn setup(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<CredentialsReq>,
) -> Result<Response, AppError> {
    validate_username(&req.username)?;
    validate_password(&req.password)?;

    let already_setup: bool = state
        .db
        .run(|conn| {
            let exists: i64 =
                conn.query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))?;
            Ok(exists != 0)
        })
        .await?;
    if already_setup {
        return Err(AppError::Conflict("Kurulum zaten tamamlanmış."));
    }

    let pw_hash = hash_password(req.password).await?;
    let token = new_session_token();
    let thash = token_hash(&token);
    let now = unix_now();
    let expires_at = now + SESSION_TTL_SECS;
    let username = req.username.clone();

    let user_id = state
        .db
        .run(move |conn| {
            let tx = conn.transaction()?;
            let inserted = tx.execute(
                "INSERT INTO users(username, password_hash, is_admin, created_at)
                 SELECT ?1, ?2, 1, ?3
                 WHERE NOT EXISTS(SELECT 1 FROM users)",
                rusqlite::params![username, pw_hash, now],
            )?;
            if inserted == 0 {
                return Err(AppError::Conflict("Kurulum zaten tamamlanmış."));
            }
            let uid = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO sessions(token_hash, user_id, created_at, expires_at, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![thash, uid, now, expires_at, now],
            )?;
            tx.commit()?;
            Ok(uid)
        })
        .await?;

    let cookie = build_session_cookie(&token, SESSION_TTL_SECS, state.cfg.cookie_secure);
    let body = UserRes {
        id: user_id,
        username: req.username,
        is_admin: true,
    };

    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, cookie)],
        Json(body),
    )
        .into_response())
}

async fn login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    JsonBody(req): JsonBody<CredentialsReq>,
) -> Result<Response, AppError> {
    if req.username.chars().count() > 32 || req.password.chars().count() > 128 {
        return Err(AppError::Unauthorized);
    }

    if let Err(retry_after) = state.limiter.reserve(&req.username, addr.ip()) {
        return Err(AppError::TooManyAttempts { retry_after });
    }

    let uname = req.username.clone();
    let row: Option<(i64, String, String, i64)> = state
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT id, username, password_hash, is_admin FROM users WHERE username = ?1",
                rusqlite::params![uname],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(AppError::from)
        })
        .await?;

    let Some((user_id, db_username, pw_hash, is_admin)) = row else {
        let _ = verify_password(req.password, DUMMY_HASH.clone()).await;
        return Err(AppError::Unauthorized);
    };

    if !verify_password(req.password, pw_hash).await {
        return Err(AppError::Unauthorized);
    }

    state.limiter.ok(&req.username, addr.ip());

    let token = new_session_token();
    let thash = token_hash(&token);
    let now = unix_now();
    let expires_at = now + SESSION_TTL_SECS;

    state
        .db
        .run(move |conn| {
            conn.execute(
                "INSERT INTO sessions(token_hash, user_id, created_at, expires_at, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![thash, user_id, now, expires_at, now],
            )?;
            Ok(())
        })
        .await?;

    let cookie = build_session_cookie(&token, SESSION_TTL_SECS, state.cfg.cookie_secure);
    let body = UserRes {
        id: user_id,
        username: db_username,
        is_admin: is_admin != 0,
    };

    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], Json(body)).into_response())
}

async fn logout(
    _user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if let Some(token) = extract_session_token(&headers) {
        let thash = token_hash(&token);
        state
            .db
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM sessions WHERE token_hash = ?1",
                    rusqlite::params![thash],
                )?;
                Ok(())
            })
            .await?;
    }

    let cookie = build_session_cookie("", 0, state.cfg.cookie_secure);
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response())
}

async fn me(user: AuthUser) -> Json<UserRes> {
    Json(UserRes {
        id: user.id,
        username: user.username,
        is_admin: user.is_admin,
    })
}

#[derive(Deserialize)]
struct ChangePasswordReq {
    current_password: String,
    new_password: String,
}

async fn change_password(
    user: AuthUser,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonBody(req): JsonBody<ChangePasswordReq>,
) -> Result<StatusCode, AppError> {
    validate_password(&req.new_password)?;

    if let Err(retry_after) = state.limiter.reserve(&user.username, addr.ip()) {
        return Err(AppError::TooManyAttempts { retry_after });
    }

    let uid = user.id;
    let pw_hash: String = state
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                rusqlite::params![uid],
                |r| r.get(0),
            )
            .map_err(AppError::from)
        })
        .await?;

    if !verify_password(req.current_password, pw_hash).await {
        return Err(AppError::Unauthorized);
    }

    state.limiter.ok(&user.username, addr.ip());

    let new_hash = hash_password(req.new_password).await?;
    let current_thash = extract_session_token(&headers)
        .map(|t| token_hash(&t))
        .unwrap_or_default();

    state
        .db
        .run(move |conn| {
            conn.execute(
                "UPDATE users SET password_hash = ?1 WHERE id = ?2",
                rusqlite::params![new_hash, uid],
            )?;
            conn.execute(
                "DELETE FROM sessions WHERE user_id = ?1 AND token_hash <> ?2",
                rusqlite::params![uid, current_thash],
            )?;
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
