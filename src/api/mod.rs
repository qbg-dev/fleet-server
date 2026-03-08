pub mod auth;
pub mod error;
pub mod models;

use axum::{Router, routing::get};
use crate::db::connection::DbPool;
use crate::config::Config;

pub fn router(db: DbPool, _config: &Config) -> Router {
    Router::new()
        .route("/health", get(health))
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "boring-mail",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
