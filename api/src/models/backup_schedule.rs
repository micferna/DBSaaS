use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BackupSchedule {
    pub id: Uuid,
    pub database_id: Uuid,
    pub interval_hours: i32,
    pub retention_count: i32,
    pub enabled: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBackupScheduleRequest {
    #[validate(range(min = 1, max = 168))]
    pub interval_hours: Option<i32>,
    #[validate(range(min = 1, max = 30))]
    pub retention_count: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBackupScheduleRequest {
    #[validate(range(min = 1, max = 168))]
    pub interval_hours: Option<i32>,
    #[validate(range(min = 1, max = 30))]
    pub retention_count: Option<i32>,
    pub enabled: Option<bool>,
}
