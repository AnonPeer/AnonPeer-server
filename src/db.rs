use sqlx::PgPool;
use shared::errors::AnonError;

pub async fn init_pool(database_url: &str) -> Result<PgPool, AnonError> {
    let pool = PgPool::connect(database_url).await
        .map_err(|e| AnonError::Db(format!("Postgres connect: {e}")))?;
        
    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        username VARCHAR(64) UNIQUE NOT NULL,
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

    Ok(pool)
}

pub async fn create_user(pool: &PgPool, username: &str, hash: &str) -> Result<(), AnonError> {
    sqlx::query("INSERT INTO users (username, password_hash) VALUES ($1, $2)")
        .bind(username).bind(hash).execute(pool).await
        .map_err(|e| AnonError::Db(format!("Create user: {e}")))?;
    Ok(())
}

pub async fn save_user_keys(pool: &PgPool, username: &str, ed_pub: &[u8], x_pub: &[u8]) -> Result<(), AnonError> {
    sqlx::query("INSERT INTO user_keys (username, ed_public, x25519_public) VALUES ($1, $2, $3) ON CONFLICT (username) DO UPDATE SET ed_public = EXCLUDED.ed_public, x25519_public = EXCLUDED.x25519_public")
        .bind(username).bind(ed_pub).bind(x_pub).execute(pool).await
        .map_err(|e| AnonError::Db(format!("Save keys: {e}")))?;
    Ok(())
}

pub async fn get_user_keys(pool: &PgPool, username: &str) -> Result<Option<(Vec<u8>, Vec<u8>)>, AnonError> {
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as("SELECT ed_public, x25519_public FROM user_keys WHERE username = $1")
        .bind(username).fetch_optional(pool).await
        .map_err(|e| AnonError::Db(format!("Get keys: {e}")))?;
    Ok(row)
}

pub async fn get_password_hash(pool: &PgPool, username: &str) -> Result<Option<String>, AnonError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE username = $1")
        .bind(username).fetch_optional(pool).await
        .map_err(|e| AnonError::Db(format!("Get hash: {e}")))?;
    Ok(row.map(|r| r.0))
}   

pub async fn user_exists(pool: &PgPool, username: &str) -> Result<bool, AnonError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT username FROM users WHERE username = $1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|e| AnonError::Db(format!("Check user: {e}")))?;
    Ok(row.is_some())
}

pub async fn save_session(pool: &PgPool, username: &str, session_id: &str) -> Result<(), AnonError> {
    sqlx::query("INSERT INTO sessions (session_id, username) VALUES ($1, $2) ON CONFLICT (session_id) DO UPDATE SET username = EXCLUDED.username")
        .bind(session_id).bind(username).execute(pool).await
        .map_err(|e| AnonError::Db(format!("Save session: {e}")))?;
    Ok(())
}

pub async fn get_username_by_session(pool: &PgPool, session_id: &str) -> Result<Option<String>, AnonError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM sessions WHERE session_id = $1")
        .bind(session_id).fetch_optional(pool).await
        .map_err(|e| AnonError::Db(format!("Get session: {e}")))?;
    Ok(row.map(|r| r.0))
}

pub async fn search_users_by_prefix(pool: &PgPool, prefix: &str) -> Result<Vec<String>, AnonError> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT username FROM users WHERE username ILIKE $1 LIMIT 15")
        .bind(format!("{}%", prefix))
        .fetch_all(pool)
        .await
        .map_err(|e| AnonError::Db(format!("Search users: {e}")))?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}