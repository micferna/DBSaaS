use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::backup_schedule::BackupSchedule;

pub struct BackupScheduleRepository;

impl BackupScheduleRepository {
    pub async fn create(
        pool: &PgPool,
        database_id: Uuid,
        interval_hours: i32,
        retention_count: i32,
        enabled: bool,
    ) -> AppResult<BackupSchedule> {
        let s = sqlx::query_as::<_, BackupSchedule>(
            "INSERT INTO backup_schedules (database_id, interval_hours, retention_count, enabled) VALUES ($1, $2, $3, $4) RETURNING id, database_id, interval_hours, retention_count, enabled, last_run_at, created_at"
        )
        .bind(database_id)
        .bind(interval_hours)
        .bind(retention_count)
        .bind(enabled)
        .fetch_one(pool)
        .await?;
        Ok(s)
    }

    pub async fn find_by_database(pool: &PgPool, database_id: Uuid) -> AppResult<Option<BackupSchedule>> {
        let s = sqlx::query_as::<_, BackupSchedule>(
            "SELECT id, database_id, interval_hours, retention_count, enabled, last_run_at, created_at FROM backup_schedules WHERE database_id = $1"
        )
        .bind(database_id)
        .fetch_optional(pool)
        .await?;
        Ok(s)
    }

    pub async fn update(
        pool: &PgPool,
        database_id: Uuid,
        interval_hours: Option<i32>,
        retention_count: Option<i32>,
        enabled: Option<bool>,
    ) -> AppResult<BackupSchedule> {
        let s = sqlx::query_as::<_, BackupSchedule>(
            "UPDATE backup_schedules SET interval_hours = COALESCE($2, interval_hours), retention_count = COALESCE($3, retention_count), enabled = COALESCE($4, enabled) WHERE database_id = $1 RETURNING id, database_id, interval_hours, retention_count, enabled, last_run_at, created_at"
        )
        .bind(database_id)
        .bind(interval_hours)
        .bind(retention_count)
        .bind(enabled)
        .fetch_one(pool)
        .await?;
        Ok(s)
    }

    pub async fn delete(pool: &PgPool, database_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM backup_schedules WHERE database_id = $1")
            .bind(database_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_last_run(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE backup_schedules SET last_run_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn find_due_schedules(pool: &PgPool) -> AppResult<Vec<BackupSchedule>> {
        let schedules = sqlx::query_as::<_, BackupSchedule>(
            "SELECT id, database_id, interval_hours, retention_count, enabled, last_run_at, created_at FROM backup_schedules WHERE enabled = true AND (last_run_at IS NULL OR last_run_at + (interval_hours || ' hours')::interval < NOW())"
        )
        .fetch_all(pool)
        .await?;
        Ok(schedules)
    }
}
