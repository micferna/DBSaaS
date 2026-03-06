use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

pub struct FavoriteRepository;

impl FavoriteRepository {
    pub async fn add(pool: &PgPool, user_id: Uuid, database_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO user_favorites (user_id, database_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
        )
        .bind(user_id)
        .bind(database_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn remove(pool: &PgPool, user_id: Uuid, database_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM user_favorites WHERE user_id = $1 AND database_id = $2")
            .bind(user_id)
            .bind(database_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Uuid>> {
        let ids: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT database_id FROM user_favorites WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(ids.into_iter().map(|(id,)| id).collect())
    }
}
