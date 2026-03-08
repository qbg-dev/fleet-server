use axum::{extract::{Path, Query, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::ListQuery;
use crate::error::ApiError;
use crate::storage::DataStore;
use serde_json::{json, Value};

/// GET /api/threads
pub async fn list_threads(
    auth: AuthAccount,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let label = q.label.as_deref().unwrap_or("INBOX");
    let list = state
        .store
        .list_threads(&auth.0.id, label, q.max_results, q.page_token.as_deref())
        .await
        .map_err(ApiError::from)?;

    let threads: Vec<Value> = list
        .threads
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "subject": t.subject,
                "snippet": t.snippet,
                "lastMessageAt": t.last_message_at,
                "messageCount": t.message_count,
                "participants": t.participants,
            })
        })
        .collect();

    Ok(Json(json!({
        "threads": threads,
        "nextPageToken": list.next_page_token,
        "resultSizeEstimate": list.result_size_estimate,
    })))
}

/// GET /api/threads/:id
pub async fn get_thread(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let thread = state.store.get_thread(&id).await.map_err(ApiError::from)?;

    let messages: Vec<Value> = thread
        .messages
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "from": m.from_account,
                "subject": m.subject,
                "body": m.body,
                "snippet": m.snippet,
                "internalDate": m.internal_date,
                "inReplyTo": m.in_reply_to,
            })
        })
        .collect();

    Ok(Json(json!({
        "id": thread.id,
        "subject": thread.subject,
        "snippet": thread.snippet,
        "lastMessageAt": thread.last_message_at,
        "messageCount": thread.message_count,
        "participants": thread.participants,
        "messages": messages,
    })))
}
