use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use crate::api::auth::{AdminAuth, AppState, AuthAccount};
use crate::api::models::{CreateAccountRequest, UpdatePaneRequest, UpdateProfileRequest};
use crate::error::ApiError;
use crate::storage::{BlobStore, DataStore};
use serde::Deserialize;
use serde_json::{json, Value};

/// POST /api/accounts — register a new account
pub async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<Value>, ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("account name cannot be empty".into()));
    }
    if name.len() > 256 {
        return Err(ApiError::BadRequest("account name too long (max 256 chars)".into()));
    }

    let account = state
        .store
        .create_account(name, req.display_name.as_deref(), req.bio.as_deref())
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint") || msg.contains("Duplicate entry") || msg.contains("duplicate unique key") {
                ApiError::Conflict(format!("account already exists: {name}"))
            } else {
                ApiError::BadRequest(msg)
            }
        })?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "displayName": account.display_name,
        "bio": account.bio,
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

/// POST /api/accounts/me/reset-token — generate a new bearer token (invalidates the old one)
pub async fn reset_token(
    auth: AuthAccount,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let account = state
        .store
        .reset_token(&auth.0.id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "bearerToken": account.bearer_token,
    })))
}

/// PUT /api/accounts/me — update own profile (display_name, bio)
pub async fn update_profile(
    auth: AuthAccount,
    State(state): State<AppState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<Value>, ApiError> {
    let account = state
        .store
        .update_profile(&auth.0.id, req.display_name.as_deref(), req.bio.as_deref())
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "displayName": account.display_name,
        "bio": account.bio,
        "active": account.active,
        "createdAt": account.created_at,
    })))
}

/// GET /api/accounts/me — shortcut for own profile
pub async fn get_account_me(
    auth: AuthAccount,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    get_account_by_id(&auth.0.id, &state).await
}

/// GET /api/accounts/:id — get account profile (own account only)
pub async fn get_account(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // Only allow viewing own account
    if id != auth.0.id {
        return Err(ApiError::Forbidden);
    }

    get_account_by_id(&id, &state).await
}

async fn get_account_by_id(id: &str, state: &AppState) -> Result<Json<Value>, ApiError> {
    let account = state
        .store
        .get_account_by_id(id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "displayName": account.display_name,
        "bio": account.bio,
        "tmuxPaneId": account.tmux_pane_id,
        "active": account.active,
        "createdAt": account.created_at,
    })))
}

/// POST /api/admin/accounts/:name/reset-token — admin resets any account's token by name
pub async fn admin_reset_token(
    _admin: AdminAuth,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let account = state
        .store
        .get_account_by_name(&name)
        .await
        .map_err(|_| ApiError::NotFound(format!("account '{name}' not found")))?;

    let account = state
        .store
        .reset_token(&account.id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": account.id,
        "name": account.name,
        "bearerToken": account.bearer_token,
    })))
}

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    pub q: Option<String>,
}

/// GET /api/directory — discover accounts (public info only, no tokens)
pub async fn directory(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Query(q): Query<DirectoryQuery>,
) -> Result<Json<Value>, ApiError> {
    let accounts = state
        .store
        .list_accounts()
        .await
        .map_err(ApiError::from)?;

    let query = q.q.map(|s| s.to_lowercase());

    let entries: Vec<Value> = accounts
        .iter()
        .filter(|a| {
            if let Some(ref q) = query {
                a.name.to_lowercase().contains(q)
                    || a.display_name.as_ref().is_some_and(|d| d.to_lowercase().contains(q))
                    || a.bio.as_ref().is_some_and(|b| b.to_lowercase().contains(q))
            } else {
                true
            }
        })
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "displayName": a.display_name,
                "bio": a.bio,
                "active": a.active,
            })
        })
        .collect();

    Ok(Json(json!({
        "directory": entries,
        "total": entries.len(),
    })))
}

/// PUT /api/accounts/me/session — upload session file (raw bytes)
pub async fn upload_session(
    auth: AuthAccount,
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty body".into()));
    }

    let meta = state
        .blobs
        .store_blob(&body)
        .await
        .map_err(ApiError::from)?;

    state
        .store
        .update_session_blob(&auth.0.id, &meta.hash)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "hash": meta.hash,
        "size": meta.size,
        "synced_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
    })))
}

/// GET /api/accounts/:name/session — download session file by account name
pub async fn download_session(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let hash = state
        .store
        .get_session_blob_hash(&name)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("no session file for account '{name}'")))?;

    let data = state
        .blobs
        .get_blob(&hash)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"session.jsonl\""),
        ],
        data,
    )
        .into_response())
}
