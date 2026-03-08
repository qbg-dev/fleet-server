use axum::{extract::State, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::error::ApiError;
use crate::storage::DataStore;
use serde_json::{json, Value};

/// GET /api/analytics — system-wide analytics
pub async fn get_analytics(
    _auth: AuthAccount,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let analytics = state
        .store
        .get_analytics()
        .await
        .map_err(ApiError::from)?;

    let per_account: Vec<Value> = analytics
        .per_account
        .iter()
        .map(|a| {
            json!({
                "accountId": a.account_id,
                "accountName": a.account_name,
                "messagesSent": a.messages_sent,
                "messagesReceived": a.messages_received,
                "threadsStarted": a.threads_started,
                "unreadCount": a.unread_count,
            })
        })
        .collect();

    Ok(Json(json!({
        "totalAccounts": analytics.total_accounts,
        "totalMessages": analytics.total_messages,
        "totalThreads": analytics.total_threads,
        "totalBlobs": analytics.total_blobs,
        "perAccount": per_account,
    })))
}
