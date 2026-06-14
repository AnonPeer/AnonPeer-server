use crate::state::AppState;
use shared::protocol::{AppMessage, ServerPayload};
use crate::services::federation;

pub async fn handle_send_message(
    mut msg: AppMessage,
    state: &AppState,
    current_user: &Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    if current_user.is_none() {
        let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Unauthorized".into())).unwrap());
        return;
    }

    let my_domain = state.server_domain.clone();
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
                federation::forward_to_federation(&target_domain, &my_domain_clone, msg).await;
            });
        }
    } else {
        let json = serde_json::to_string(&ServerPayload::Forward { msg: msg.clone() }).unwrap();
        if let Some(peer_tx) = state.peers.get(&msg.to) {
            let _ = peer_tx.value().send(json);
        }
    }
}

pub async fn handle_federate_message(
    msg: AppMessage,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    let my_domain = state.server_domain.clone();
    if msg.to.ends_with(&format!("@{}", my_domain)) || msg.to.ends_with("@localhost") {
        let local_user = msg.to.split('@').next().unwrap_or(&msg.to).to_string();
        let json = serde_json::to_string(&ServerPayload::Forward { msg }).unwrap();
        if let Some(peer_tx) = state.peers.get(&local_user) {
            let _ = peer_tx.value().send(json);
        }
    }
}