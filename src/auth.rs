use sqlx::PgPool;
use shared::crypto;
use shared::errors::AnonError;
use uuid::Uuid;

pub async fn register(pool: &PgPool, username: &str, password: &str) -> Result<String, AnonError> {
    let hash = crypto::hash_password(password)?;
    crate::db::create_user(pool, username, &hash).await?;
    Ok(Uuid::new_v4().to_string())
}

pub async fn login(pool: &PgPool, username: &str, password: &str) -> Result<String, AnonError> {
    let hash = crate::db::get_password_hash(pool, username).await?
        .ok_or_else(|| AnonError::Auth("User not found".into()))?;
    if crypto::verify_password(password, &hash)? {
        Ok(Uuid::new_v4().to_string())
    } else {
        Err(AnonError::Auth("Invalid password".into()))
    }
}