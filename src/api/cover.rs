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
        cover::{
            encode_custom_cover, publish_tmp, render_cover, write_tmp, BookRow, COVER_ENCODE,
        },
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
    // kodlama sıraya girer: aynı anda tek görsel çözülür
    let jpeg = {
        let _slot = COVER_ENCODE
            .acquire()
            .await
            .map_err(|_| AppError::Internal)?;
        tokio::task::spawn_blocking(move || encode_custom_cover(&body))
            .await
            .map_err(|_| AppError::Internal)?
            .map_err(|()| AppError::BadRequest("Görsel okunamadı."))?
    };

    let covers = state.cfg.data_dir.join("covers");
    let dst = covers.join(format!("{id}.jpg"));
    let dir = covers.clone();
    let tmp = tokio::task::spawn_blocking(move || write_tmp(&dir, id, &jpeg))
        .await
        .map_err(|_| AppError::Internal)?
        .map_err(|e| {
            tracing::error!("özel kapak yazılamadı: {e}");
            AppError::Internal
        })?;

    // yayımlama ve veritabanı güncellemesi tarama/DELETE ile aynı kilit altında
    let _guard = state.cover_lock.lock().await;
    if let Err(e) = publish_tmp(&tmp, &dst) {
        tracing::error!("özel kapak yayımlanamadı: {e}");
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
    let _guard = state.cover_lock.lock().await;
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
    let dst = covers.join(format!("{id}.jpg"));
    let has_cover = match render_cover(&state.cfg.books_dir, &row, &covers).await {
        Some(tmp) => publish_tmp(&tmp, &dst).is_ok(),
        None => false,
    };
    if !has_cover {
        let _ = std::fs::remove_file(covers.join(format!("{id}.jpg")));
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
