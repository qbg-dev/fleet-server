use axum::{extract::{Path, Query, State}, Json};
use crate::api::auth::{AppState, AuthAccount};
use crate::api::models::{BatchModifyRequest, ListQuery, ModifyLabelsRequest, SendMessageRequest};
use crate::delivery::tmux;
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

    // Expand mailing list recipients into individual account IDs
    let expanded_to = expand_list_recipients(&state.store, &req.to, &auth.0.id).await;

    // Capture for notification before moving into NewMessage
    let notify_recipients_list = expanded_to.clone();
    let notify_subject = req.subject.clone();
    let notify_from = auth.0.name.clone();

    let msg = NewMessage {
        from_account: auth.0.id,
        to: expanded_to,
        cc: req.cc,
        subject: req.subject,
        body: req.body,
        thread_id: req.thread_id,
        in_reply_to: req.in_reply_to,
        reply_by: req.reply_by,
        labels: req.labels,
        source: req.source,
        attachments: req.attachments,
    };

    let sent = state.store.insert_message(msg).await.map_err(ApiError::from)?;

    // Fire-and-forget tmux notifications to recipients
    let store = state.store.clone();
    tokio::spawn(async move {
        notify_recipients(&store, &notify_recipients_list, &notify_from, &notify_subject).await;
    });

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

    // Fetch attachments if present
    let attachments = if msg.has_attachments {
        state.store.get_attachments(&id).await.unwrap_or_default()
    } else {
        vec![]
    };

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
        "attachments": attachments.iter().map(|a| json!({
            "blobHash": a.blob_hash,
            "filename": a.filename,
            "contentType": a.content_type,
            "size": a.size,
        })).collect::<Vec<_>>(),
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

/// Expand mailing list names in recipients to individual subscriber account IDs.
/// List names are prefixed with "list:" (e.g., "list:team-updates").
/// Deduplicates against explicit recipients and excludes the sender.
async fn expand_list_recipients(
    store: &crate::storage::sqlite::SqliteDataStore,
    recipients: &[String],
    sender_id: &str,
) -> Vec<String> {
    let mut expanded: Vec<String> = Vec::new();

    for recipient in recipients {
        if let Some(list_name) = recipient.strip_prefix("list:") {
            // Look up list and expand to members
            if let Ok((list_id, _, _)) = store.get_list_by_name(list_name).await {
                if let Ok(members) = store.get_list_members(&list_id).await {
                    for member in members {
                        // Skip sender (no self-delivery) and dedup
                        if member != sender_id && !expanded.contains(&member) {
                            expanded.push(member);
                        }
                    }
                }
            } else {
                // Not a list, pass through as-is
                expanded.push(recipient.clone());
            }
        } else {
            if !expanded.contains(recipient) {
                expanded.push(recipient.clone());
            }
        }
    }

    expanded
}

/// Look up each recipient's tmux pane and send a notification.
async fn notify_recipients(
    store: &crate::storage::sqlite::SqliteDataStore,
    recipients: &[String],
    from: &str,
    subject: &str,
) {
    for recipient_id in recipients {
        let account = match store.get_account_by_name(recipient_id).await {
            Ok(a) => a,
            Err(_) => continue,
        };

        if let Some(ref pane_id) = account.tmux_pane_id {
            tmux::notify_new_messages(pane_id, 1, from, subject);
        }
    }
}
