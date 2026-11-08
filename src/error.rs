use axum::{
    extract::{rejection::JsonRejection, FromRequest, Request},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub enum AppError {
    BadRequest(&'static str),
    Unauthorized,
    Forbidden,
    Csrf,
    NotFound,
    Conflict(&'static str),
    PayloadTooLarge,
    UnsupportedMedia,
    Corrupt,
    Drm,
    TooManyAttempts { retry_after: u64 },
    Internal,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, retry_after) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg, None),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Oturum açmanız gerekiyor.",
                None,
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "Bu işlem için yetkiniz yok.",
                None,
            ),
            AppError::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf",
                "Geçersiz istek kaynağı.",
                None,
            ),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", "Bulunamadı.", None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg, None),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "İstek gövdesi çok büyük.",
                None,
            ),
            AppError::UnsupportedMedia => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media",
                "Content-Type application/json olmalıdır.",
                None,
            ),
            AppError::Corrupt => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "corrupt_book",
                "Bu dosya bozuk veya okunamıyor.",
                None,
            ),
            AppError::Drm => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "drm_protected",
                "Bu kitap DRM korumalı olduğu için açılamıyor.",
                None,
            ),
            AppError::TooManyAttempts { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_attempts",
                "Çok fazla başarısız deneme. Lütfen daha sonra tekrar deneyin.",
                Some(retry_after),
            ),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Sunucu hatası.",
                None,
            ),
        };

        let body = serde_json::to_string(&ErrorBody {
            error: code,
            message,
        })
        .unwrap_or_else(|_| r#"{"error":"internal","message":"Sunucu hatası."}"#.to_string());

        let mut res = (
            status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            body,
        )
            .into_response();

        if let Some(secs) = retry_after {
            if let Ok(val) = header::HeaderValue::from_str(&secs.to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, val);
            }
        }

        res
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        tracing::error!("veritabanı hatası: {err}");
        AppError::Internal
    }
}

pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(val)) => Ok(JsonBody(val)),
            Err(rejection) => match rejection {
                JsonRejection::MissingJsonContentType(_) => Err(AppError::UnsupportedMedia),
                JsonRejection::BytesRejection(b) if b.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                    Err(AppError::PayloadTooLarge)
                }
                _ => Err(AppError::BadRequest("Geçersiz JSON gövdesi.")),
            },
        }
    }
}
