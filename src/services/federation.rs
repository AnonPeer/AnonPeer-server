use shared::protocol::{ClientPayload, ServerPayload};
use tokio_tungstenite::connect_async;
use futures_util::{StreamExt, SinkExt};
use tracing;

pub async fn fetch_federated_keys(target_domain: &str, target_user: &str, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            let payload = ClientPayload::RequestKeys { target: target_user.to_string() };
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

pub async fn forward_to_federation(target_domain: &str, my_domain: &str, msg: shared::protocol::AppMessage) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, _) = ws_stream.split();
            let payload = ClientPayload::Federate {
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

pub async fn search_federation_peer(target_domain: &str, prefix: &str, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            let payload = ClientPayload::SearchPrefix { prefix: prefix.to_string() };
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

pub async fn fetch_federated_profile(target_domain: &str, username: &str, tx: tokio::sync::mpsc::UnboundedSender<String>) {
    let ws_url = format!("ws://{}/ws", target_domain);
    match connect_async(&ws_url).await {
        Ok((ws_stream, _)) => {
            let (mut ws_sender, mut ws_receiver) = ws_stream.split();
            let local_name = username.split('@').next().unwrap_or(username);
            let payload = ClientPayload::RequestProfile { username: local_name.to_string() };
            if let Ok(json) = serde_json::to_string(&payload) {
                let _ = ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await;
                if let Some(Ok(msg)) = ws_receiver.next().await {
                    if let Ok(text) = msg.to_text() {
                        if let Ok(ServerPayload::ProfileResult { user: Some(mut profile) }) = serde_json::from_str::<ServerPayload>(text) {
                            profile.server_domain = Some(target_domain.to_string());
                            let _ = tx.send(serde_json::to_string(&ServerPayload::ProfileResult { user: Some(profile) }).unwrap());
                        }
                    }
                }
            }
        }
        Err(e) => tracing::error!("Failed to fetch federated profile from {}: {}", target_domain, e),
    }
}