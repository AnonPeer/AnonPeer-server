use sqlx::PgPool;
use shared::errors::AnonError;
use shared::protocol::UserInfo;

pub async fn create_user(pool: &PgPool, username: &str, nickname: &str, hash: &str) -> Result<(), AnonError> {
    sqlx::query("INSERT INTO users (username, nickname, password_hash) VALUES ($1, $2, $3)")
        .bind(username).bind(nickname).bind(hash).execute(pool).await
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
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE username = $1")
        .bind(username).fetch_optional(pool).await
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

pub async fn search_users_by_prefix(pool: &PgPool, prefix: &str) -> Result<Vec<UserInfo>, AnonError> {
    let search_pattern = format!("%{}%", prefix);
    let rows: Vec<(String, String, String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT nickname, username, COALESCE(bio,''), avatar_base64, last_seen FROM users WHERE username ILIKE $1 OR nickname ILIKE $1 LIMIT 15"
    )
    .bind(&search_pattern)
    .fetch_all(pool)
    .await
    .map_err(|e| AnonError::Db(format!("Search users: {e}")))?;
    Ok(rows.into_iter().map(|(nickname, username, bio, avatar, last_seen)| UserInfo {
        nickname, username, bio,
        avatar_base64: avatar,
        server_domain: None,
        last_seen: last_seen.map(|v| v as u64),
    }).collect())
}

pub async fn get_user_profile(pool: &PgPool, username: &str) -> Result<Option<UserInfo>, AnonError> {
    let row: Option<(String, String, String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT nickname, username, COALESCE(bio,''), avatar_base64, last_seen FROM users WHERE username = $1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|e| AnonError::Db(format!("Get profile: {e}")))?;
    Ok(row.map(|(nickname, username, bio, avatar, last_seen)| UserInfo {
        nickname, username, bio,
        avatar_base64: avatar,
        server_domain: None,
        last_seen: last_seen.map(|v| v as u64),
    }))
}

pub async fn update_user_profile(pool: &PgPool, username: &str, bio: Option<&str>, avatar: Option<&str>) -> Result<(), AnonError> {
    if let Some(bio) = bio {
        sqlx::query("UPDATE users SET bio = $1 WHERE username = $2")
            .bind(bio).bind(username).execute(pool).await
            .map_err(|e| AnonError::Db(format!("Update bio: {e}")))?;
    }
    if let Some(avatar) = avatar {
        sqlx::query("UPDATE users SET avatar_base64 = $1 WHERE username = $2")
            .bind(avatar).bind(username).execute(pool).await
            .map_err(|e| AnonError::Db(format!("Update avatar: {e}")))?;
    }
    Ok(())
}

pub async fn update_last_seen(pool: &PgPool, username: &str) -> Result<(), AnonError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    sqlx::query("UPDATE users SET last_seen = $1 WHERE username = $2")
        .bind(now).bind(username).execute(pool).await
        .map_err(|e| AnonError::Db(format!("Update last_seen: {e}")))?;
    Ok(())
}