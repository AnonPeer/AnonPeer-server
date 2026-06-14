use crate::state::AppState;
use crate::services::{auth, message, profile};
use shared::protocol::{ClientPayload, ServerPayload};
use tracing;

pub async fn dispatch(
    payload: ClientPayload,
    state: &AppState,
    current_user: &mut Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    let respond = |payload: ServerPayload| {
        if let Ok(json) = serde_json::to_string(&payload) {
            let _ = tx.send(json);
        }
    };

    match payload {
        ClientPayload::Register { nickname, username, password, ed_public, x25519_public } => {
            match auth::register(&state.pool, &nickname, &username, &password).await {
                Ok(session) => {
                    let _ = crate::db::queries::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                    let _ = crate::db::queries::save_session(&state.pool, &username, &session).await;
                    *current_user = Some(username.clone());
                    state.peers.insert(username, tx.clone());
                    respond(ServerPayload::AuthOk { session_id: session });
                }
                Err(e) => respond(ServerPayload::AuthErr(e.to_string())),
            }
        }
        ClientPayload::Login { username, password, ed_public, x25519_public } => {
            match auth::login(&state.pool, &username, &password).await {
                Ok(session) => {
                    let _ = crate::db::queries::save_user_keys(&state.pool, &username, &ed_public, &x25519_public).await;
                    let _ = crate::db::queries::save_session(&state.pool, &username, &session).await;
                    let _ = crate::db::queries::update_last_seen(&state.pool, &username).await;
                    *current_user = Some(username.clone());
                    state.peers.insert(username.clone(), tx.clone());
                    tracing::info!("User {} connected", username);
                    respond(ServerPayload::AuthOk { session_id: session });
                }
                Err(_) => respond(ServerPayload::AuthErr("Login failed".into())),
            }
        }
        ClientPayload::ValidateSession { session_id } => {
            match auth::validate_session(&state.pool, &session_id).await {
                Ok(Some(username)) => {
                    *current_user = Some(username.clone());
                    state.peers.insert(username.clone(), tx.clone());
                    tracing::info!("User {} reconnected via session", username);
                    respond(ServerPayload::AuthOk { session_id });
                }
                _ => respond(ServerPayload::AuthErr("Недействительная сессия".into())),
            }
        }
        ClientPayload::SendMessage { msg } => {
            message::handle_send_message(msg, state, current_user, tx).await;
        }
        ClientPayload::Federate { from_server: _, msg } => {
            message::handle_federate_message(msg, state, tx).await;
        }
        ClientPayload::RequestKeys { target } => {
            profile::handle_request_keys(target, state, tx).await;
        }
        ClientPayload::SearchPrefix { prefix } => {
            profile::handle_search_prefix(prefix, state, tx).await;
        }
        ClientPayload::RequestProfile { username } => {
            profile::handle_request_profile(username, state, current_user, tx).await;
        }
        ClientPayload::UpdateProfile { bio, avatar_base64 } => {
            if let Some(ref user) = current_user {
                let _ = crate::db::queries::update_user_profile(&state.pool, user, bio.as_deref(), avatar_base64.as_deref()).await;
                respond(ServerPayload::ProfileUpdated);
            }
        }
    }
}