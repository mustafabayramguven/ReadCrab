use axum::Router;

use crate::app::AppState;

pub mod admin;
pub mod auth;
pub mod bookmarks;
pub mod library;
pub mod reader;
pub mod stats;
pub mod tracking;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(admin::router())
        .merge(library::router())
        .merge(bookmarks::router())
        .merge(reader::router())
        .merge(tracking::router())
        .merge(stats::router())
}
