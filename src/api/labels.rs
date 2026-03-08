use axum::{extract::{Path, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::error::ApiError;
use crate::storage::DataStore;
use serde::Deserialize;
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

#[derive(Debug, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
}

/// POST /api/labels — create a custom label
pub async fn create_label(
    auth: AuthAccount,
    State(state): State<AppState>,
    Json(req): Json<CreateLabelRequest>,
) -> Result<Json<Value>, ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("label name cannot be empty".into()));
    }
    if name.len() > 256 {
        return Err(ApiError::BadRequest("label name too long (max 256 chars)".into()));
    }

    let label = state
        .store
        .create_label(&auth.0.id, &req.name)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": label.0,
        "name": label.1,
        "type": "user",
    })))
}

/// DELETE /api/labels/:name — delete a custom label
pub async fn delete_label(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .delete_label(&auth.0.id, &name)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({"deleted": true})))
}
