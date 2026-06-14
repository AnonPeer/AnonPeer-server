pub mod connection;
pub mod dispatcher;

use axum::{Router, routing::get};
use crate::state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(connection::ws_handler))
        .with_state(state)
}