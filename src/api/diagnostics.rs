use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Response},
    middleware::Next,
};
use crate::api::auth::AppState;
use crate::storage::DataStore;

/// Middleware that appends `_diagnostics` to JSON responses for authenticated users.
pub async fn diagnostics_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response<Body> {
    // Extract bearer token before passing request to handler
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let response = next.run(request).await;

    // Only modify successful JSON responses from authenticated requests
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("application/json"));

    let is_success = response.status().is_success();

    let Some(token) = token else {
        return response;
    };

    if !is_json || !is_success {
        return response;
    }

    // Get account from token
    let account = match state.store.get_account_by_token(&token).await {
        Ok(a) => a,
        Err(_) => return response,
    };

    let unread_count = state.store.get_unread_count(&account.id).await.unwrap_or(0);
    let pending_replies = state
        .store
        .get_pending_replies(&account.id)
        .await
        .unwrap_or_default();
    let overdue_count = pending_replies
        .iter()
        .filter(|p| {
            p.reply_by.as_ref().is_some_and(|rb| {
                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                rb < &now
            })
        })
        .count() as u32;

    let inbox_hint = if unread_count > 0 {
        Some(format!(
            "You have {unread_count} unread messages. Use GET /api/messages?label=UNREAD to read them."
        ))
    } else {
        None
    };

    // Read the response body
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    // Parse and inject _diagnostics
    let mut json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return Response::from_parts(parts, Body::from(bytes)),
    };

    if let Some(obj) = json.as_object_mut() {
        let pending: Vec<serde_json::Value> = pending_replies
            .iter()
            .map(|p| {
                serde_json::json!({
                    "messageId": p.message_id,
                    "from": p.from_account,
                    "subject": p.subject,
                    "replyBy": p.reply_by,
                })
            })
            .collect();

        obj.insert(
            "_diagnostics".to_string(),
            serde_json::json!({
                "unread_count": unread_count,
                "pending_replies": pending,
                "overdue_count": overdue_count,
                "inbox_hint": inbox_hint,
            }),
        );
    }

    let new_bytes = serde_json::to_vec(&json).unwrap_or_else(|_| bytes.to_vec());
    let len = new_bytes.len();
    let mut response = Response::from_parts(parts, Body::from(new_bytes));
    if let Ok(len) = len.try_into() {
        response.headers_mut().insert(header::CONTENT_LENGTH, len);
    }
    response
}
