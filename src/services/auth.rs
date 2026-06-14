use sqlx::PgPool;
use shared::crypto;
use shared::errors::AnonError;
use uuid::Uuid;
use crate::db::queries;

pub async fn register(pool: &PgPool, nickname: &str, username: &str, password: &str) -> Result<String, AnonError> {
    if queries::user_exists(pool, username).await? {
        return Err(AnonError::Auth("User already exists".into()));
    }
    let hash = crypto::hash_password(password)?;
    queries::create_user(pool, username, nickname, &hash).await?;
    Ok(Uuid::new_v4().to_string())
}

pub async fn login(pool: &PgPool, username: &str, password: &str) -> Result<String, AnonError> {
    let hash = queries::get_password_hash(pool, username).await?
        .ok_or_else(|| AnonError::Auth("User not found".into()))?;
    if crypto::verify_password(password, &hash)? {
        Ok(Uuid::new_v4().to_string())
    } else {
        Err(AnonError::Auth("Invalid password".into()))
    }
}

pub async fn validate_session(pool: &PgPool, session_id: &str) -> Result<Option<String>, AnonError> {
    queries::get_username_by_session(pool, session_id).await
}