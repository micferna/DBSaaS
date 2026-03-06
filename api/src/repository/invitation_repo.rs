use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::Invitation;

pub struct InvitationRepository;

impl InvitationRepository {
    pub async fn create(
        pool: &PgPool,
        code: &str,
        created_by: Uuid,
        max_uses: i32,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<Invitation> {
        let inv = sqlx::query_as::<_, Invitation>(
            r#"INSERT INTO invitations (code, created_by, max_uses, expires_at)
               VALUES ($1, $2, $3, $4)
               RETURNING id, code, created_by, used_by, max_uses, use_count, expires_at, created_at"#,
        )
        .bind(code)
        .bind(created_by)
        .bind(max_uses)
        .bind(expires_at)
        .fetch_one(pool)
        .await?;

        Ok(inv)
    }

    pub async fn find_by_code(pool: &PgPool, code: &str) -> AppResult<Option<Invitation>> {
        let inv = sqlx::query_as::<_, Invitation>(
            "SELECT id, code, created_by, used_by, max_uses, use_count, expires_at, created_at FROM invitations WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;

        Ok(inv)
    }

    pub async fn use_invitation(pool: &PgPool, code: &str, user_id: Uuid) -> AppResult<bool> {
        let result = sqlx::query(
            r#"UPDATE invitations
               SET use_count = use_count + 1, used_by = $1
               WHERE code = $2
                 AND use_count < max_uses
                 AND (expires_at IS NULL OR expires_at > NOW())"#,
        )
        .bind(user_id)
        .bind(code)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<Invitation>> {
        let invitations = sqlx::query_as::<_, Invitation>(
            "SELECT id, code, created_by, used_by, max_uses, use_count, expires_at, created_at FROM invitations ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(invitations)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM invitations WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
