use axum::Router;

use crate::app::AppState;

pub mod auth;

pub fn router() -> Router<AppState> {
    Router::new().merge(auth::router())
}
