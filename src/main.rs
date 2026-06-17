use std::env;
use tracing_subscriber;

mod db;
mod state;
mod services;
mod ws;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();
    let _ = dotenvy::dotenv();
    
    let db_url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL not set")?;
    let pool = db::init_pool(&db_url).await?;
    
    let app_state = state::AppState::new(pool);
    let app = ws::app(app_state);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    
    tracing::info!("Server listening on :{}", port);
    axum::serve(listener, app).await?;
    
    Ok(())
}

/*
             . --- .
           /        \
          |  O  _  O |
          |  ./   \. |
          /  `-._.-'  \
        .' /         \ `.
    .-~.-~/           \~-.~-.
.-~ ~    |             |    ~ ~-.
`- .     |             |     . -'
     ~ - |             | - ~
         \             /
       ___\           /___
       ~;_  &gt;- . . -&lt;  _i~
          `'         `'
The server code is guarded by this penguin, please don't break it.
*/

