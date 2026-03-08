use axum::{extract::{Path, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::{CreateAccountRequest, UpdatePaneRequest};
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

/// GET /api/accounts/:id/pending — recycle readiness check
pub async fn pending(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let id = if id == "me" { auth.0.id.clone() } else { id };

    let pending = state
        .store
        .get_pending_replies(&id)
        .await
        .map_err(ApiError::from)?;

    let unanswered: Vec<Value> = pending
        .iter()
        .map(|p| {
            json!({
                "messageId": p.message_id,
                "from": p.from_account,
                "subject": p.subject,
                "replyBy": p.reply_by,
            })
        })
        .collect();

    let ready = unanswered.is_empty();
    let blockers: Vec<String> = if !ready {
        vec![format!("{} unanswered reply requests", unanswered.len())]
    } else {
        vec![]
    };

    Ok(Json(json!({
        "unanswered_requests": unanswered,
        "ready_to_recycle": ready,
        "blockers": blockers,
    })))
}

/// POST /api/accounts/:id/pane — register tmux pane for notifications
pub async fn update_pane(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePaneRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = if id == "me" { auth.0.id.clone() } else { id };

    state
        .store
        .update_pane(&id, &req.pane_id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": id,
        "tmuxPaneId": req.pane_id,
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
