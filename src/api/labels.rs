use axum::{extract::State, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::error::ApiError;
use crate::storage::DataStore;
use serde_json::{json, Value};

/// GET /api/labels — list labels with counts
pub async fn list_labels(
    auth: AuthAccount,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let labels = state
        .store
        .list_labels_with_counts(&auth.0.id)
        .await
        .map_err(ApiError::from)?;

    let labels_json: Vec<Value> = labels
        .iter()
        .map(|l| {
            json!({
                "name": l.name,
                "type": l.label_type,
                "messagesTotal": l.message_count,
                "messagesUnread": l.unread_count,
            })
        })
        .collect();

    Ok(Json(json!({ "labels": labels_json })))
}
