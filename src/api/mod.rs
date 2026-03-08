pub mod analytics;
pub mod auth;
pub mod diagnostics;
pub mod error;
pub mod models;
pub mod accounts;
pub mod blobs;
pub mod labels;
pub mod lists;
pub mod messages;
pub mod search;
pub mod threads;
pub mod webhooks;

use axum::{Router, middleware, routing::{get, post, delete}};
use crate::db::connection::DbPool;
use crate::config::Config;
use crate::storage::sqlite::SqliteDataStore;
use crate::storage::fts::SqliteSearchStore;
use crate::storage::blob::FsBlobStore;
use auth::AppState;

pub fn router(db: DbPool, config: &Config) -> Router {
    let state = AppState {
        store: SqliteDataStore::new(db.clone()),
        search: SqliteSearchStore::new(db),
        blobs: FsBlobStore::new(config),
    };

    Router::new()
        // Health (no auth)
        .route("/health", get(health))
        // Accounts (create is unauthenticated)
        .route("/api/accounts", post(accounts::create_account))
        .route("/api/accounts/{id}", get(accounts::get_account))
        .route("/api/accounts/{id}/pane", post(accounts::update_pane))
        .route("/api/accounts/{id}/pending", get(accounts::pending))
        // Messages
        .route("/api/messages/send", post(messages::send_message))
        .route("/api/messages", get(messages::list_messages))
        .route("/api/messages/{id}", get(messages::get_message))
        .route("/api/messages/{id}", delete(messages::delete_message))
        .route("/api/messages/{id}/modify", post(messages::modify_message))
        .route("/api/messages/{id}/trash", post(messages::trash_message))
        .route("/api/messages/batchModify", post(messages::batch_modify))
        // Labels
        .route("/api/labels", get(labels::list_labels).post(labels::create_label))
        .route("/api/labels/{name}", delete(labels::delete_label))
        // Threads
        .route("/api/threads", get(threads::list_threads))
        .route("/api/threads/{id}", get(threads::get_thread))
        // Search
        .route("/api/search", get(search::search))
        // Mailing Lists
        .route("/api/lists", post(lists::create_list))
        .route("/api/lists/{id}/subscribe", post(lists::subscribe))
        .route("/api/lists/{id}/unsubscribe", post(lists::unsubscribe))
        // Blobs
        .route("/api/blobs", post(blobs::upload_blob))
        .route("/api/blobs/{hash}", get(blobs::download_blob))
        // Analytics
        .route("/api/analytics", get(analytics::get_analytics))
        // Webhooks
        .route("/api/webhooks/git-commit", post(webhooks::git_commit))
        // Diagnostics middleware
        .layer(middleware::from_fn_with_state(
            state.clone(),
            diagnostics::diagnostics_middleware,
        ))
        .with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "boring-mail",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
