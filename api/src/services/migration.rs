use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::MigrationRecord;

pub struct MigrationService;

impl MigrationService {
    pub async fn execute_migration(
        host: &str,
        port: i32,
        username: &str,
        password: &str,
        database: &str,
        sql: &str,
    ) -> AppResult<()> {
        let url = format!("postgres://{username}:{password}@{host}:{port}/{database}");
        let pool = PgPool::connect(&url)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to client DB: {e}")))?;

        sqlx::query(sql)
            .execute(&pool)
            .await
            .map_err(|e| AppError::BadRequest(format!("Migration failed: {e}")))?;

        pool.close().await;
        Ok(())
    }

    pub async fn record_migration(
        pool: &PgPool,
        database_id: Uuid,
        filename: &str,
        checksum: &str,
    ) -> AppResult<MigrationRecord> {
        let record = sqlx::query_as::<_, MigrationRecord>(
            r#"INSERT INTO migration_records (database_id, filename, checksum)
               VALUES ($1, $2, $3)
               RETURNING id, database_id, filename, checksum, applied_at"#,
        )
        .bind(database_id)
        .bind(filename)
        .bind(checksum)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    pub async fn list_migrations(
        pool: &PgPool,
        database_id: Uuid,
    ) -> AppResult<Vec<MigrationRecord>> {
        let records = sqlx::query_as::<_, MigrationRecord>(
            "SELECT id, database_id, filename, checksum, applied_at FROM migration_records WHERE database_id = $1 ORDER BY applied_at",
        )
        .bind(database_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }
}
