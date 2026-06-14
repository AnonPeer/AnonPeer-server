use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use axum::extract::State;
use futures_util::{StreamExt, SinkExt};
use crate::state::AppState;
use super::dispatcher;
use shared::protocol::ClientPayload;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_connection(socket, state))
}

async fn handle_connection(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut current_user: Option<String> = None;

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() { break; }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let text = match msg.to_text() { Ok(t) => t, Err(_) => continue };
        if text.is_empty() { continue; }

        let Ok(payload) = serde_json::from_str::<ClientPayload>(text) else {
            tracing::warn!("Invalid JSON payload"); continue;
        };

        dispatcher::dispatch(payload, &state, &mut current_user, &tx).await;
    }

    if let Some(user) = current_user {
        state.peers.remove(&user);
        tracing::info!("User {} disconnected", user);
    }
    send_task.abort();
}