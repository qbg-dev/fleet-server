use axum::{extract::{Path, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::error::ApiError;
use crate::storage::DataStore;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct CreateListRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// POST /api/lists — create a mailing list
pub async fn create_list(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Json(req): Json<CreateListRequest>,
) -> Result<Json<Value>, ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("list name cannot be empty".into()));
    }
    if name.len() > 256 {
        return Err(ApiError::BadRequest("list name too long (max 256 chars)".into()));
    }

    let id = state
        .store
        .create_list(name, &req.description)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(Json(json!({
        "id": id,
        "name": req.name,
        "description": req.description,
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct SubscribeRequest {
    /// Account to subscribe. Defaults to the authenticated caller.
    pub account_id: Option<String>,
}

/// POST /api/lists/:id/subscribe
pub async fn subscribe(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<SubscribeRequest>>,
) -> Result<Json<Value>, ApiError> {
    let target = body
        .and_then(|Json(r)| r.account_id)
        .unwrap_or_else(|| auth.0.id.clone());
    state
        .store
        .subscribe_to_list(&id, &target)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({"subscribed": true})))
}

/// POST /api/lists/:id/unsubscribe
pub async fn unsubscribe(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .unsubscribe_from_list(&id, &auth.0.id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({"unsubscribed": true})))
}
