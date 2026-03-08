use axum::{extract::{Path, Query, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::{BatchModifyRequest, ListQuery, ModifyLabelsRequest, SendMessageRequest};
use crate::error::ApiError;
use crate::storage::models::NewMessage;
use crate::storage::DataStore;
use serde_json::{json, Value};

/// POST /api/messages/send
pub async fn send_message(
    auth: AuthAccount,
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<Value>, ApiError> {
    if req.to.is_empty() {
        return Err(ApiError::BadRequest("empty recipients".into()));
    }

    let msg = NewMessage {
        from_account: auth.0.id,
        to: req.to,
        cc: req.cc,
        subject: req.subject,
        body: req.body,
        thread_id: req.thread_id,
        in_reply_to: req.in_reply_to,
        reply_by: req.reply_by,
        labels: req.labels,
        source: req.source,
    };

    let sent = state.store.insert_message(msg).await.map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": sent.id,
        "threadId": sent.thread_id,
        "labelIds": sent.labels,
    })))
}

/// GET /api/messages
pub async fn list_messages(
    auth: AuthAccount,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let label = q.label.as_deref().unwrap_or("INBOX");
    let list = state
        .store
        .list_messages(&auth.0.id, label, q.max_results, q.page_token.as_deref())
        .await
        .map_err(ApiError::from)?;

    let messages: Vec<Value> = list
        .messages
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "threadId": m.thread_id,
                "snippet": m.snippet,
                "internalDate": m.internal_date,
                "from": m.from_account,
                "subject": m.subject,
            })
        })
        .collect();

    Ok(Json(json!({
        "messages": messages,
        "nextPageToken": list.next_page_token,
        "resultSizeEstimate": list.result_size_estimate,
    })))
}

/// GET /api/messages/:id
pub async fn get_message(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let msg = state.store.get_message(&id).await.map_err(ApiError::from)?;

    // Auto-remove UNREAD for the authenticated user
    state
        .store
        .remove_labels(&id, &auth.0.id, &["UNREAD".to_string()])
        .await
        .ok(); // ignore if no UNREAD label

    Ok(Json(json!({
        "id": msg.id,
        "threadId": msg.thread_id,
        "from": msg.from_account,
        "to": msg.recipients.iter().filter(|r| r.recipient_type == "to").map(|r| &r.account_id).collect::<Vec<_>>(),
        "cc": msg.recipients.iter().filter(|r| r.recipient_type == "cc").map(|r| &r.account_id).collect::<Vec<_>>(),
        "subject": msg.subject,
        "body": msg.body,
        "snippet": msg.snippet,
        "labelIds": msg.labels,
        "internalDate": msg.internal_date,
        "inReplyTo": msg.in_reply_to,
        "replyBy": msg.reply_by,
        "replyRequested": msg.reply_requested,
        "hasAttachments": msg.has_attachments,
        "source": msg.source,
    })))
}

/// POST /api/messages/:id/modify
pub async fn modify_message(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ModifyLabelsRequest>,
) -> Result<Json<Value>, ApiError> {
    // Verify message exists
    state.store.get_message(&id).await.map_err(ApiError::from)?;

    if !req.add_label_ids.is_empty() {
        state
            .store
            .add_labels(&id, &auth.0.id, &req.add_label_ids)
            .await
            .map_err(ApiError::from)?;
    }
    if !req.remove_label_ids.is_empty() {
        state
            .store
            .remove_labels(&id, &auth.0.id, &req.remove_label_ids)
            .await
            .map_err(ApiError::from)?;
    }

    let labels = state
        .store
        .get_labels(&id, &auth.0.id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": id,
        "labelIds": labels,
    })))
}

/// POST /api/messages/:id/trash
pub async fn trash_message(
    auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.store.get_message(&id).await.map_err(ApiError::from)?;

    state
        .store
        .remove_labels(&id, &auth.0.id, &["INBOX".to_string()])
        .await
        .map_err(ApiError::from)?;
    state
        .store
        .add_labels(&id, &auth.0.id, &["TRASH".to_string()])
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "id": id,
        "labelIds": ["TRASH"],
    })))
}

/// POST /api/messages/batchModify
pub async fn batch_modify(
    auth: AuthAccount,
    State(state): State<AppState>,
    Json(req): Json<BatchModifyRequest>,
) -> Result<Json<Value>, ApiError> {
    if req.ids.is_empty() {
        return Err(ApiError::BadRequest("empty ids".into()));
    }

    state
        .store
        .batch_modify_labels(
            &req.ids,
            &auth.0.id,
            &req.add_label_ids,
            &req.remove_label_ids,
        )
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({"modified": req.ids.len()})))
}

/// DELETE /api/messages/:id
pub async fn delete_message(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.store.delete_message(&id).await.map_err(ApiError::from)?;
    Ok(Json(json!({"deleted": true})))
}
