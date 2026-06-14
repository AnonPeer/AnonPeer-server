pub mod queries;
use sqlx::PgPool;
use shared::errors::AnonError;

pub async fn init_pool(database_url: &str) -> Result<PgPool, AnonError> {
    let pool = PgPool::connect(database_url).await
        .map_err(|e| AnonError::Db(format!("Postgres connect: {e}")))?;
    
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        username VARCHAR(64) UNIQUE NOT NULL,
        nickname TEXT NOT NULL,
        password_hash TEXT NOT NULL
    )").execute(&pool).await.map_err(|e| AnonError::Db(format!("Init users: {e}")))?;

    sqlx::query("CREATE TABLE IF NOT EXISTS user_keys (
        username TEXT PRIMARY KEY,
        ed_public BYTEA NOT NULL,
        x25519_public BYTEA NOT NULL
    )").execute(&pool).await.map_err(|e| AnonError::Db(format!("Init keys: {e}")))?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sessions (
        session_id TEXT PRIMARY KEY,
        username TEXT NOT NULL
    )").execute(&pool).await.map_err(|e| AnonError::Db(format!("Init sessions: {e}")))?;

    let _ = sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS bio TEXT DEFAULT ''").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_base64 TEXT").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS last_seen BIGINT").execute(&pool).await;

    Ok(pool)
}