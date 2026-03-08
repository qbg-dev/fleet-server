use axum::{extract::{Path, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::CreateAccountRequest;
use crate::error::ApiError;
use crate::storage::DataStore;
use serde_json::{json, Value};

/// POST /api/accounts — register a new account
pub async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<Value>, ApiError> {
    let account = state
        .store
        .create_account(&req.name, req.display_name.as_deref())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "displayName": account.display_name,
        "bearerToken": account.bearer_token,
        "active": account.active,
        "createdAt": account.created_at,
    })))
}

/// GET /api/accounts/:id — get account profile
pub async fn get_account(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let id = if id == "me" { _auth.0.id.clone() } else { id };

    let account = state
        .store
        .get_account_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "displayName": account.display_name,
        "tmuxPaneId": account.tmux_pane_id,
        "active": account.active,
        "createdAt": account.created_at,
    })))
}
