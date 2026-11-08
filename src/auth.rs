use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, Request},
    http::{header, request::Parts, HeaderMap, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

use crate::{app::AppState, error::AppError};

pub const SESSION_COOKIE_NAME: &str = "krs_session";
pub const SESSION_TTL_SECS: i64 = 30 * 86400;
const WINDOW_SECS: u64 = 15 * 60;
const USER_MAX_FAILS: usize = 5;
const IP_MAX_FAILS: usize = 20;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Clone, Debug)]
pub struct AdminUser(pub AuthUser);

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub async fn hash_password(pw: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(pw.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| {
                tracing::error!("parola özetleme hatası: {e}");
                AppError::Internal
            })
    })
    .await
    .map_err(|_| AppError::Internal)?
}

pub async fn verify_password(pw: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok(parsed) = PasswordHash::new(&hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(pw.as_bytes(), &parsed)
            .is_ok()
    })
    .await
    .unwrap_or(false)
}

pub fn new_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    for val in headers.get_all(header::COOKIE) {
        let Ok(s) = val.to_str() else { continue };
        for part in s.split(';') {
            let trimmed = part.trim();
            if let Some(tok) = trimmed.strip_prefix("krs_session=") {
                if !tok.is_empty() {
                    return Some(tok.to_string());
                }
            }
        }
    }
    None
}

pub fn build_session_cookie(token: &str, max_age: i64, secure: bool) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_session_token(&parts.headers).ok_or(AppError::Unauthorized)?;
        let thash = token_hash(&token);
        let now = unix_now();

        state
            .db
            .run(move |conn| {
                let row: Option<(i64, String, i64, i64)> = conn
                    .query_row(
                        "SELECT u.id, u.username, u.is_admin, s.last_seen
                         FROM sessions s
                         JOIN users u ON u.id = s.user_id
                         WHERE s.token_hash = ?1 AND s.expires_at > ?2",
                        rusqlite::params![thash, now],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()?;

                let Some((id, username, is_admin, last_seen)) = row else {
                    return Err(AppError::Unauthorized);
                };

                if now - last_seen >= 60 {
                    let new_exp = now + SESSION_TTL_SECS;
                    conn.execute(
                        "UPDATE sessions SET last_seen = ?1, expires_at = ?2 WHERE token_hash = ?3",
                        rusqlite::params![now, new_exp, thash],
                    )?;
                }

                Ok(AuthUser {
                    id,
                    username,
                    is_admin: is_admin != 0,
                })
            })
            .await
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}

#[derive(Default)]
struct Bucket {
    failures: Vec<Instant>,
    locked_until: Option<Instant>,
}

impl Bucket {
    fn prune(&mut self, now: Instant, window: Duration) {
        if let Some(until) = self.locked_until {
            if now >= until {
                self.locked_until = None;
            }
        }
        self.failures.retain(|&t| now.duration_since(t) < window);
    }

    fn remaining_lock(&self, now: Instant) -> Option<u64> {
        self.locked_until.and_then(|until| {
            if until > now {
                Some(until.duration_since(now).as_secs().max(1))
            } else {
                None
            }
        })
    }

    fn record_failure(&mut self, now: Instant, window: Duration, limit: usize) {
        self.prune(now, window);
        self.failures.push(now);
        if self.failures.len() >= limit {
            self.locked_until = Some(now + window);
        }
    }
}

#[derive(Default)]
pub struct LoginLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl LoginLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&self, user: &str, ip: IpAddr) -> Result<(), u64> {
        let now = Instant::now();
        let window = Duration::from_secs(WINDOW_SECS);
        let ukey = format!("u:{}", user.to_ascii_lowercase());
        let ikey = format!("ip:{ip}");

        let mut map = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let mut retry = None;

        if let Some(b) = map.get_mut(&ukey) {
            b.prune(now, window);
            if let Some(secs) = b.remaining_lock(now) {
                retry = Some(secs);
            }
        }
        if let Some(b) = map.get_mut(&ikey) {
            b.prune(now, window);
            if let Some(secs) = b.remaining_lock(now) {
                retry = Some(retry.map_or(secs, |r: u64| r.max(secs)));
            }
        }

        match retry {
            Some(secs) => Err(secs),
            None => Ok(()),
        }
    }

    pub fn fail(&self, user: &str, ip: IpAddr) {
        let now = Instant::now();
        let window = Duration::from_secs(WINDOW_SECS);
        let ukey = format!("u:{}", user.to_ascii_lowercase());
        let ikey = format!("ip:{ip}");

        let mut map = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(ukey)
            .or_default()
            .record_failure(now, window, USER_MAX_FAILS);
        map.entry(ikey)
            .or_default()
            .record_failure(now, window, IP_MAX_FAILS);
    }

    pub fn ok(&self, user: &str) {
        let ukey = format!("u:{}", user.to_ascii_lowercase());
        let mut map = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&ukey);
    }
}

pub async fn origin_and_content_type_guard(req: Request, next: Next) -> Response {
    let is_mutating = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    );

    if is_mutating {
        if let Some(origin_val) = req.headers().get(header::ORIGIN) {
            let Ok(origin_str) = origin_val.to_str() else {
                return AppError::Csrf.into_response();
            };
            let Some(rest) = origin_str
                .strip_prefix("http://")
                .or_else(|| origin_str.strip_prefix("https://"))
            else {
                return AppError::Csrf.into_response();
            };
            let origin_host = rest.split('/').next().unwrap_or("");
            let host_str = req
                .headers()
                .get(header::HOST)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            if origin_host.is_empty() || !origin_host.eq_ignore_ascii_case(host_str) {
                return AppError::Csrf.into_response();
            }
        }

        let ct_ok = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"));

        if !ct_ok {
            return AppError::UnsupportedMedia.into_response();
        }
    }

    next.run(req).await
}

pub fn validate_username(username: &str) -> Result<(), AppError> {
    let len = username.len();
    if !(3..=32).contains(&len) {
        return Err(AppError::BadRequest(
            "Kullanıcı adı 3-32 karakter olmalıdır.",
        ));
    }
    if !username
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return Err(AppError::BadRequest(
            "Kullanıcı adı yalnızca harf, rakam ve _.- içerebilir.",
        ));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    let len = password.chars().count();
    if !(8..=128).contains(&len) {
        return Err(AppError::BadRequest("Parola 8-128 karakter olmalıdır."));
    }
    Ok(())
}
