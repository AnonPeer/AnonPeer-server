use sqlx::PgPool;
use dashmap::DashMap;
use std::sync::Arc;

pub type PeerMap = Arc<DashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub peers: PeerMap,
    pub server_domain: String,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        let server_domain = std::env::var("SERVER_DOMAIN").unwrap_or_else(|_| "localhost".to_string());
        Self {
            pool,
            peers: Arc::new(DashMap::new()),
            server_domain,
        }
    }
}