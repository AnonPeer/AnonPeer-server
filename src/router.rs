use std::sync::Arc;
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use axum::{Router, routing::get, extract::State};
use sqlx::PgPool;
use dashmap::DashMap;
use shared::protocol::{ClientPayload, ServerPayload};
use futures_util::{StreamExt, SinkExt};

type PeerMap = Arc<DashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub peers: PeerMap,
}

pub fn app(pool: PgPool) -> Router {
    let peers: PeerMap = Arc::new(DashMap::new());
    let state = AppState { pool, peers };
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut current_user: Option<String> = None;

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() { break; }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let text = match msg.to_text() {
            Ok(t) => t,
            Err(_) => continue, 
        };
        if text.is_empty() { continue; }
        
        let payload: Result<ClientPayload, _> = serde_json::from_str(text);
        let Ok(payload) = payload else { continue; };

        match payload {
            ClientPayload::Register { username, password, ed_public, x25519_public } => {
                let res = crate::auth::register(&state.pool, &username, &password).await;
                let out = match res {
                    Ok(session) => {
                        let _ = crate::db::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                        current_user = Some(username.clone());
                        state.peers.insert(username, tx.clone());
                        ServerPayload::AuthOk { session_id: session }
                    },
                    Err(e) => ServerPayload::AuthErr(e.to_string()),
                };
                let _ = tx.send(serde_json::to_string(&out).unwrap());
            }
            ClientPayload::Login { username, password, ed_public, x25519_public } => {
                let res = crate::auth::login(&state.pool, &username, &password).await;
                if let Ok(session) = res {
                    let _ = crate::db::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                    current_user = Some(username.clone());
                    state.peers.insert(username.clone(), tx.clone());
                    tracing::info!("User {} connected", username);
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthOk { session_id: session }).unwrap());
                } else {
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Login failed".into())).unwrap());
                }
            }
            ClientPayload::SendMessage { msg } => {
                if current_user.is_none() {
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Unauthorized".into())).unwrap());
                    continue;
                }
                let json = serde_json::to_string(&ServerPayload::Forward { msg: msg.clone() }).unwrap();
                if let Some(peer_tx) = state.peers.get(&msg.to) {
                    let _ = peer_tx.value().send(json);
                }
            }
            ClientPayload::RequestKeys { target } => {
                if current_user.is_none() {
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Unauthorized".into())).unwrap());
                    continue;
                }
                if let Ok(Some((ed_pub, x_pub))) = crate::db::get_user_keys(&state.pool, &target).await {
                    let resp = ServerPayload::PeerKeys { target, ed_public: ed_pub, x25519_public: x_pub };
                    let _ = tx.send(serde_json::to_string(&resp).unwrap());
                }
            }
        }
    }

    if let Some(user) = current_user {
        state.peers.remove(&user);
        tracing::info!("User {} disconnected", user);
    }
    send_task.abort();
}