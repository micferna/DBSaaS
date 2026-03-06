use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AlertRule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub database_id: Option<Uuid>,
    pub event_type: String,
    pub webhook_url: Option<String>,
    pub email: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AlertHistory {
    pub id: Uuid,
    pub alert_rule_id: Uuid,
    pub event_type: String,
    pub message: String,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAlertRuleRequest {
    pub database_id: Option<Uuid>,
    pub event_type: String,
    pub webhook_url: Option<String>,
    pub email: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertRuleRequest {
    pub webhook_url: Option<String>,
    pub email: Option<String>,
    pub enabled: Option<bool>,
}
