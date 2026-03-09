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
    let account = state
        .store
        .get_account_by_token(&params.token)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    if !account.active {
        return Err(ApiError::Forbidden);
    }

    let rx = state.events_tx.subscribe();
    let account_id = account.id.clone();

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, rx, account_id)))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<MailEvent>,
    account_id: String,
) {
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) if event.account_id == account_id => {
                        let json = serde_json::to_string(&event).unwrap_or_default();
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    _ => {} // skip events for other accounts, or lagged
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
