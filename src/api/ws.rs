use axum::{
    extract::{Query, State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::IntoResponse,
};
use serde::Deserialize;
use crate::api::auth::{AppState, MailEvent};
use crate::error::ApiError;
use crate::storage::DataStore;

#[derive(Deserialize)]
pub struct WsParams {
    token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Admin token → unfiltered mode (receives ALL events for all accounts)
    let is_admin = state.admin_token.as_deref() == Some(&params.token);

    let account_id_filter = if is_admin {
        None
    } else {
        let account = state
            .store
            .get_account_by_token(&params.token)
            .await
            .map_err(|_| ApiError::Unauthorized)?;

        if !account.active {
            return Err(ApiError::Forbidden);
        }

        Some(account.id.clone())
    };

    let rx = state.events_tx.subscribe();

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, rx, account_id_filter)))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<MailEvent>,
    account_id_filter: Option<String>,
) {
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let should_forward = match &account_id_filter {
                            None => true, // admin: forward all events
                            Some(id) => event.account_id == *id,
                        };
                        if should_forward {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    _ => {} // lagged — skip
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
