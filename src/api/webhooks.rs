use axum::{extract::State, Json};
use crate::api::auth::AppState;
use crate::error::ApiError;
use crate::storage::models::NewMessage;
use crate::storage::DataStore;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct GitCommitWebhook {
    pub author: String,
    pub sha: String,
    pub message: String,
    #[serde(default)]
    pub recipients: Vec<String>,
}

/// POST /api/webhooks/git-commit — deliver commit notification as mail
pub async fn git_commit(
    State(state): State<AppState>,
    Json(req): Json<GitCommitWebhook>,
) -> Result<Json<Value>, ApiError> {
    // Look up author account by name
    let author = state
        .store
        .get_account_by_name(&req.author)
        .await
        .map_err(|_| ApiError::BadRequest(format!("unknown author: {}", req.author)))?;

    let recipients = if req.recipients.is_empty() {
        // If no recipients specified, skip (future: broadcast to a list)
        return Ok(Json(json!({"delivered": 0})));
    } else {
        req.recipients
    };

    let short_sha = &req.sha[..7.min(req.sha.len())];
    let msg = NewMessage {
        from_account: author.id,
        to: recipients,
        cc: vec![],
        subject: format!("commit: {}", req.message),
        body: format!("{} committed {}\n\n{}", req.author, short_sha, req.message),
        thread_id: None,
        in_reply_to: None,
        reply_by: None,
        labels: vec!["COMMIT".to_string()],
        source: Some("webhook:git-commit".to_string()),
        attachments: vec![],
    };

    let sent = state.store.insert_message(msg).await.map_err(ApiError::from)?;

    Ok(Json(json!({
        "delivered": 1,
        "messageId": sent.id,
    })))
}
