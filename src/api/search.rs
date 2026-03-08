use axum::{extract::{Query, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::ListQuery;
use crate::error::ApiError;
use crate::storage::DataStore;
use crate::storage::SearchStore;
use serde_json::{json, Value};

/// GET /api/search?q=query
pub async fn search(
    auth: AuthAccount,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let query = q.q.ok_or_else(|| ApiError::BadRequest("missing q parameter".into()))?;

    let ids = state
        .search
        .search(&auth.0.id, &query, q.max_results)
        .await
        .map_err(ApiError::from)?;

    // Fetch full messages for the result IDs
    let mut messages = Vec::new();
    for id in &ids {
        let msg = state.store.get_message(id).await.map_err(ApiError::from)?;
        messages.push(json!({
            "id": msg.id,
            "threadId": msg.thread_id,
            "from": msg.from_account,
            "subject": msg.subject,
            "snippet": msg.snippet,
            "internalDate": msg.internal_date,
        }));
    }

    Ok(Json(json!({
        "messages": messages,
        "resultSizeEstimate": messages.len(),
    })))
}
