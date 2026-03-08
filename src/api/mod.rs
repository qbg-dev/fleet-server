pub mod auth;
pub mod error;
pub mod models;
pub mod accounts;
pub mod messages;
pub mod labels;
pub mod threads;

use axum::{Router, routing::{get, post, delete}};
use crate::db::connection::DbPool;
use crate::config::Config;
use crate::storage::sqlite::SqliteDataStore;
use auth::AppState;

pub fn router(db: DbPool, _config: &Config) -> Router {
    let state = AppState {
        store: SqliteDataStore::new(db),
    };

    Router::new()
        // Health (no auth)
        .route("/health", get(health))
        // Accounts (create is unauthenticated)
        .route("/api/accounts", post(accounts::create_account))
        .route("/api/accounts/{id}", get(accounts::get_account))
        // Messages
        .route("/api/messages/send", post(messages::send_message))
        .route("/api/messages", get(messages::list_messages))
        .route("/api/messages/{id}", get(messages::get_message))
        .route("/api/messages/{id}", delete(messages::delete_message))
        .route("/api/messages/{id}/modify", post(messages::modify_message))
        .route("/api/messages/{id}/trash", post(messages::trash_message))
        // Labels
        .route("/api/labels", get(labels::list_labels))
        // Threads
        .route("/api/threads", get(threads::list_threads))
        .route("/api/threads/{id}", get(threads::get_thread))
        .with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "boring-mail",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
