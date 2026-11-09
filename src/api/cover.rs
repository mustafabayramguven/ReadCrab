use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    routing::put,
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::{
    app::AppState,
    auth::AdminUser,
    error::AppError,
    formats::{
        cover::{encode_custom_cover, extract_cover, BookRow},
        Format,
    },
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/books/{id}/cover",
        put(put_cover)
            .delete(delete_cover)
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
    )
}

#[derive(Serialize)]
struct CoverVersionRes {
    cover_version: i64,
}

#[derive(Serialize)]
struct CoverDeleteRes {
    cover_version: i64,
    has_cover: bool,
}

async fn book_row(state: &AppState, id: i64, allow_missing: bool) -> Result<BookRow, AppError> {
    let row: Option<(String, String, i64)> = state
        .db
        .run(move |conn| {
            conn.query_row(
                "SELECT rel_path, format, missing FROM books WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(AppError::from)
        })
        .await?;
    let Some((rel_path, fmt, missing)) = row else {
        return Err(AppError::NotFound);
    };
    if missing != 0 && !allow_missing {
        return Err(AppError::NotFound);
    }
    let format = Format::from_ext(&fmt).ok_or(AppError::Internal)?;
    Ok(BookRow {
        id,
        rel_path,
        format,
    })
}

async fn put_cover(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Bytes,
) -> Result<Json<CoverVersionRes>, AppError> {
    book_row(&state, id, false).await?;
    let jpeg = tokio::task::spawn_blocking(move || encode_custom_cover(&body))
        .await
        .map_err(|_| AppError::Internal)?
        .map_err(|()| AppError::BadRequest("Görsel okunamadı."))?;

    let covers = state.cfg.data_dir.join("covers");
    let tmp = covers.join(format!("{id}.jpg.tmp"));
    let dst = covers.join(format!("{id}.jpg"));
    let written = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&covers)?;
        std::fs::write(&tmp, jpeg)?;
        std::fs::rename(&tmp, &dst)
    })
    .await
    .map_err(|_| AppError::Internal)?;
    if let Err(e) = written {
        tracing::error!("özel kapak yazılamadı: {e}");
        return Err(AppError::Internal);
    }

    let cover_version = state
        .db
        .run(move |conn| {
            conn.execute(
                "UPDATE books SET has_cover = 1, custom_cover = 1, cover_version = cover_version + 1
                 WHERE id = ?1",
                [id],
            )?;
            let sql = "SELECT cover_version FROM books WHERE id = ?1";
            Ok(conn.query_row(sql, [id], |r| r.get(0))?)
        })
        .await?;
    Ok(Json(CoverVersionRes { cover_version }))
}

async fn delete_cover(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CoverDeleteRes>, AppError> {
    let row = book_row(&state, id, true).await?;
    state
        .db
        .run(move |conn| {
            conn.execute(
                "UPDATE books SET custom_cover = 0, cover_version = cover_version + 1
                 WHERE id = ?1",
                [id],
            )?;
            Ok(())
        })
        .await?;

    let covers = state.cfg.data_dir.join("covers");
    let has_cover = extract_cover(&state.cfg.books_dir, &row, &covers).await;
    if !has_cover {
        let _ = tokio::fs::remove_file(covers.join(format!("{id}.jpg"))).await;
    }
    let cover_version = state
        .db
        .run(move |conn| {
            conn.execute(
                "UPDATE books SET has_cover = ?2 WHERE id = ?1",
                rusqlite::params![id, has_cover],
            )?;
            let sql = "SELECT cover_version FROM books WHERE id = ?1";
            Ok(conn.query_row(sql, [id], |r| r.get(0))?)
        })
        .await?;
    Ok(Json(CoverDeleteRes {
        cover_version,
        has_cover,
    }))
}
