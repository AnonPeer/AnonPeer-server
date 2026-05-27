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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server listening on :3000");

    axum::serve(listener, app).await?;
    Ok(())
}