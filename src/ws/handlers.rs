use super::AppState;
use shared::protocol::{ClientPayload, ServerPayload, AppMessage};
use crate::{auth, db};

pub async fn handle_payload(
    payload: ClientPayload,
    state: &AppState,
    current_user: &mut Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    match payload {
        ClientPayload::Register { nickname, username, password, ed_public, x25519_public } => {
            let res = auth::service::register(&state.pool, &nickname, &username, &password).await;
            let out = match res {
                Ok(session) => {
                    let _ = db::queries::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                    let _ = db::queries::save_session(&state.pool, &username, &session).await;
                    *current_user = Some(username.clone());
                    state.peers.insert(username, tx.clone());
                    ServerPayload::AuthOk { session_id: session }
                }
                Err(e) => ServerPayload::AuthErr(e.to_string()),
            };
            let _ = tx.send(serde_json::to_string(&out).unwrap());
        }
        ClientPayload::SendMessage { msg } => {
            handle_send_message(msg, state, current_user, tx).await;
        }
        _ => tracing::warn!("Unhandled payload"),
    }
}

async fn handle_send_message(
    mut msg: AppMessage,
    state: &AppState,
    current_user: &Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    if current_user.is_none() {
        let _ = tx.send(serde_json::to_string(&ServerPayload::AuthErr("Unauthorized".into())).unwrap());
        return;
    }
}