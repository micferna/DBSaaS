use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{User, UserRole};

/// Hash an API key for secure storage/lookup (timing-safe comparison via DB)
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    // Format as hex string manually
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub struct UserRepository;

impl UserRepository {
    pub async fn create(
        pool: &PgPool,
        email: &str,
        password_hash: &str,
        role: &UserRole,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"INSERT INTO users (email, password_hash, role)
               VALUES ($1, $2, $3)
               RETURNING id, email, password_hash, role, api_key, max_databases, created_at, updated_at"#,
        )
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, role, api_key, max_databases, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, role, api_key, max_databases, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_api_key(pool: &PgPool, api_key: &str) -> AppResult<Option<User>> {
        let key_hash = hash_api_key(api_key);
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, role, api_key, max_databases, created_at, updated_at FROM users WHERE api_key = $1",
        )
        .bind(key_hash)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Store the SHA-256 hash of the API key (the plaintext is returned once to the user)
    pub async fn set_api_key(pool: &PgPool, user_id: Uuid, api_key: &str) -> AppResult<()> {
        let key_hash = hash_api_key(api_key);
        sqlx::query("UPDATE users SET api_key = $1, updated_at = NOW() WHERE id = $2")
            .bind(key_hash)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, role, api_key, max_databases, created_at, updated_at FROM users ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(users)
    }

    pub async fn update_role(pool: &PgPool, user_id: Uuid, role: &UserRole) -> AppResult<()> {
        sqlx::query("UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2")
            .bind(role)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_max_databases(pool: &PgPool, user_id: Uuid, max: i32) -> AppResult<()> {
        sqlx::query("UPDATE users SET max_databases = $1, updated_at = NOW() WHERE id = $2")
            .bind(max)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
