use crate::state::AppState;
use crate::db::queries;
use crate::services::federation;
use shared::protocol::ServerPayload;

pub async fn handle_request_keys(
    target: String,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    let my_domain = state.server_domain.clone();
    let local_user = if target.contains('@') {
        target.split('@').next().unwrap_or(&target).to_string()
    } else {
        target.clone()
    };

    if target.contains('@') {
        let parts: Vec<&str> = target.splitn(2, '@').collect();
        let target_domain = parts[1].to_string();

        if target_domain == my_domain {
            if let Ok(Some((ed_pub, x_pub))) = queries::get_user_keys(&state.pool, &local_user).await {
                let resp = ServerPayload::PeerKeys { target, ed_public: ed_pub, x25519_public: x_pub };
                let _ = tx.send(serde_json::to_string(&resp).unwrap());
            }
        } else {
            let tx_clone = tx.clone();
            let target_clone = target.clone();
            tokio::spawn(async move {
                federation::fetch_federated_keys(&target_domain, &target_clone, tx_clone).await;
            });
        }
    } else {
        if let Ok(Some((ed_pub, x_pub))) = queries::get_user_keys(&state.pool, &target).await {
            let resp = ServerPayload::PeerKeys { target, ed_public: ed_pub, x25519_public: x_pub };
            let _ = tx.send(serde_json::to_string(&resp).unwrap());
        }
    }
}

pub async fn handle_search_prefix(
    prefix: String,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    if prefix.len() < 2 { return; }

    let my_domain = state.server_domain.clone();
    let federation_peers = std::env::var("FEDERATION_PEERS").unwrap_or_else(|_| String::new());

    let mut local_matches = queries::search_users_by_prefix(&state.pool, &prefix).await.unwrap_or_default();
    for m in &mut local_matches {
        m.server_domain = Some(my_domain.clone());
    }

    let mut all_matches = local_matches.clone();
    let mut search_tasks = vec![];

    for peer in federation_peers.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if peer == my_domain { continue; }

        let prefix_clone = prefix.clone();
        let peer_clone = peer.to_string();
        let tx_clone = tx.clone();

        let task = tokio::spawn(async move {
            federation::search_federation_peer(&peer_clone, &prefix_clone, tx_clone).await;
        });
        search_tasks.push(task);
    }

    for task in search_tasks {
        let _ = task.await;
    }

    all_matches.sort_by(|a, b| a.username.cmp(&b.username));
    all_matches.dedup_by(|a, b| a.username == b.username && a.server_domain == b.server_domain);

    let resp = ServerPayload::SearchResults { matches: all_matches };
    let _ = tx.send(serde_json::to_string(&resp).unwrap());
}

pub async fn handle_request_profile(
    username: String,
    state: &AppState,
    current_user: &Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    if let Some(ref user) = current_user {
        let _ = queries::update_last_seen(&state.pool, user).await;
    }
    let my_domain = state.server_domain.clone();
    if username.contains('@') {
        let parts: Vec<&str> = username.splitn(2, '@').collect();
        let target_domain = parts[1].to_string();
        if target_domain != my_domain {
            let tx_clone = tx.clone();
            let uname = username.clone();
            tokio::spawn(async move {
                federation::fetch_federated_profile(&target_domain, &uname, tx_clone).await;
            });
            return;
        }
    }
    let local_name = username.split('@').next().unwrap_or(&username);
    let mut profile = queries::get_user_profile(&state.pool, local_name).await.unwrap_or(None);
    if let Some(ref mut p) = profile {
        p.server_domain = Some(my_domain.clone());
    }
    let resp = ServerPayload::ProfileResult { user: profile };
    let _ = tx.send(serde_json::to_string(&resp).unwrap());
}