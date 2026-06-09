use std::env;
use tracing_subscriber;
mod db;
mod auth;
mod router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();
    let _ = dotenvy::dotenv();
    let db_url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL not set")?;
    
    let pool = db::init_pool(&db_url).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let app = router::app(pool);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Server listening on :{}", port);

    axum::serve(listener, app).await?;
    Ok(())
}