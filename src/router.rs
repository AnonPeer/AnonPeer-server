use std::sync::Arc;
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use axum::{Router, routing::get, extract::State};
use sqlx::PgPool;
use dashmap::DashMap;
use shared::protocol::{ClientPayload, ServerPayload, AppMessage, UserInfo};
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

            ClientPayload::Register { nickname, username, password, ed_public, x25519_public } => {
                let res = crate::auth::register(&state.pool, &nickname, &username, &password).await;
                let out = match res {
                    Ok(session) => {
                        let _ = crate::db::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                        let _ = crate::db::save_session(&state.pool, &username, &session).await;
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
                    let _ = crate::db::save_session(&state.pool, &username, &session).await;
                    current_user = Some(username.clone());
                    state.peers.insert(username.clone(), tx.clone());
                    tracing::info!("User {} connected", username);
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthOk { session_id: session }).unwrap());
                } else {
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Login failed".into())).unwrap());
                }
            }
            ClientPayload::Federate { from_server: _, msg } => {
                let my_domain = std::env::var("SERVER_DOMAIN").unwrap_or_else(|_| "localhost".to_string());
                if msg.to.ends_with(&format!("@{}", my_domain)) || msg.to.ends_with("@localhost") {
                    let local_user = msg.to.split('@').next().unwrap_or(&msg.to).to_string();
                    let json = serde_json::to_string(&ServerPayload::Forward { msg }).unwrap();
                    if let Some(peer_tx) = state.peers.get(&local_user) {
                        let _ = peer_tx.value().send(json);
                    }
                }
            }
            ClientPayload::SendMessage { mut msg } => {
                if current_user.is_none() {
                    let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Unauthorized".into())).unwrap());
                    continue;
                }
                
                let my_domain = std::env::var("SERVER_DOMAIN").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
                if !msg.from.contains('@') {
                    msg.from = format!("{}@{}", msg.from, my_domain);
                }

                if msg.to.contains('@') {
                    let parts: Vec<&str> = msg.to.splitn(2, '@').collect();
                    let target_domain = parts[1].to_string();
                    
                    if target_domain == my_domain {
                        let local_user = parts[0].to_string();
                        let json = serde_json::to_string(&ServerPayload::Forward { msg }).unwrap();
                        if let Some(peer_tx) = state.peers.get(&local_user) {
                            let _ = peer_tx.value().send(json);
                        }
                    } else {
                        let state_clone = state.clone();
                        let my_domain_clone = my_domain.clone();
                        tokio::spawn(async move {
                            forward_to_federation(&target_domain, &my_domain_clone, msg, state_clone).await;
                        });
                    }
                } else {
                    let json = serde_json::to_string(&ServerPayload::Forward { msg: msg.clone() }).unwrap();
                    if let Some(peer_tx) = state.peers.get(&msg.to) {
                        let _ = peer_tx.value().send(json);
                    }
                }
            }
            ClientPayload::RequestKeys { target } => {
                let my_domain = std::env::var("SERVER_DOMAIN").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
                let local_user = if target.contains('@') {
                    target.split('@').next().unwrap_or(&target).to_string()
                } else {
                    target.clone()
                };
                
                if target.contains('@') {
                    let parts: Vec<&str> = target.splitn(2, '@').collect();
                    let target_domain = parts[1].to_string();

                    if target_domain == my_domain {
                        if let Ok(Some((ed_pub, x_pub))) = crate::db::get_user_keys(&state.pool, &local_user).await {
                            let resp = ServerPayload::PeerKeys { target, ed_public: ed_pub, x25519_public: x_pub };
                            let _ = tx.send(serde_json::to_string(&resp).unwrap());
                        }
                    } else {
                        let tx_clone = tx.clone();
                        let target_clone = target.clone();
                        tokio::spawn(async move {
                            fetch_federated_keys(&target_domain, &target_clone, tx_clone).await;
                        });
                    }
                } else {
                    if let Ok(Some((ed_pub, x_pub))) = crate::db::get_user_keys(&state.pool, &target).await {
                        let resp = ServerPayload::PeerKeys { target, ed_public: ed_pub, x25519_public: x_pub };
                        let _ = tx.send(serde_json::to_string(&resp).unwrap());
                    }
                }
            }

            ClientPayload::SearchPrefix { prefix } => {
                if prefix.len() < 2 { continue; }
                
                let my_domain = std::env::var("SERVER_DOMAIN").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
                let federation_peers = std::env::var("FEDERATION_PEERS").unwrap_or_else(|_| String::new());
                
                let local_matches = crate::db::search_users_by_prefix(&state.pool, &prefix).await.unwrap_or_default();
                
                let mut all_matches = local_matches.clone();
                let mut search_tasks = vec![];
                
                for peer in federation_peers.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if peer == my_domain { continue; }
                    
                    let prefix_clone = prefix.clone();
                    let peer_clone = peer.to_string();
                    let tx_clone = tx.clone();
                    
                    let task = tokio::spawn(async move {
                        search_federation_peer(&peer_clone, &prefix_clone, tx_clone).await;
                    });
                    search_tasks.push(task);
                }
                
                for task in search_tasks {
                    let _ = task.await;
                }
                
                all_matches.sort_by(|a, b| a.username.cmp(&b.username));
                all_matches.dedup_by(|a, b| a.username == b.username);
                
                let resp = ServerPayload::SearchResults { matches: all_matches };
                let _ = tx.send(serde_json::to_string(&resp).unwrap());
            }

            ClientPayload::RequestProfile { username } => {
                let profile = crate::db::get_user_profile(&state.pool, &username).await.unwrap_or(None);
                let resp = ServerPayload::ProfileResult { user: profile };
                let _ = tx.send(serde_json::to_string(&resp).unwrap());
            }
            ClientPayload::ValidateSession { session_id } => { 
                match crate::db::get_username_by_session(&state.pool, &session_id).await {
                    Ok(Some(username)) => {
                        current_user = Some(username.clone());
                        state.peers.insert(username.clone(), tx.clone());
                        tracing::info!("User {} reconnected via session", username);
                        let _ = tx.send(serde_json::to_string(&ServerPayload::AuthOk { session_id: session_id.clone() }).unwrap());
                    }
                    _ => {
                        let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Недействительная сессия".into())).unwrap());
                    }
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

async fn fetch_federated_keys(target_domain: &str, target_user: &str, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match tokio_tungstenite::connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            let payload = shared::protocol::ClientPayload::RequestKeys { target: target_user.to_string() };
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await;
                if let Some(Ok(msg)) = ws_receiver.next().await {
                    if let Ok(text) = msg.to_text() {
                        let _ = tx.send(text.to_string());
                    }
                }
            }
        }
        Err(e) => tracing::error!("Failed to connect to federation peer for keys {}: {}", target_domain, e),
    }
}

async fn forward_to_federation(target_domain: &str, my_domain: &str, msg: shared::protocol::AppMessage, _state: AppState) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match tokio_tungstenite::connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, _) = ws_stream.split();
            let payload = shared::protocol::ClientPayload::Federate {
                from_server: my_domain.to_string(),
                msg,
            };
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await;
            }
        }
        Err(e) => tracing::error!("Failed to connect to federation peer {}: {}", target_domain, e),
    }
}

async fn search_federation_peer(target_domain: &str, prefix: &str, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match tokio_tungstenite::connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            let payload = shared::protocol::ClientPayload::SearchPrefix { prefix: prefix.to_string() };
            if let Ok(json) = serde_json::to_string(&payload) {
                if ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await.is_err() { return; }
                if let Some(Ok(msg)) = ws_receiver.next().await {
                    if let Ok(text) = msg.to_text() {
                        if let Ok(ServerPayload::SearchResults { matches: _ }) = serde_json::from_str::<ServerPayload>(text) {
                            let _ = tx.send(text.to_string());
                        }
                    }
                }
            }
        }
        Err(e) => tracing::error!("Failed to connect to federation peer {}: {}", target_domain, e),
    }
}