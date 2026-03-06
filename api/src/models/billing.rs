use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::DbType;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlanTemplate {
    pub id: Uuid,
    pub name: String,
    pub db_type: DbType,
    pub cpu_limit: f64,
    pub memory_limit_mb: i32,
    pub monthly_price_cents: i32,
    pub hourly_price_cents: i32,
    pub is_bundle: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlanTemplateRequest {
    pub name: String,
    pub db_type: DbType,
    pub cpu_limit: f64,
    pub memory_limit_mb: i32,
    pub monthly_price_cents: i32,
    pub hourly_price_cents: i32,
    #[serde(default)]
    pub is_bundle: bool,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlanTemplateRequest {
    pub name: Option<String>,
    pub cpu_limit: Option<f64>,
    pub memory_limit_mb: Option<i32>,
    pub monthly_price_cents: Option<i32>,
    pub hourly_price_cents: Option<i32>,
    pub is_bundle: Option<bool>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UsageEvent {
    pub id: Uuid,
    pub database_id: Uuid,
    pub event_type: String,
    pub recorded_at: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub database_name: Option<String>,
    pub plan_template_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BillingPeriod {
    pub id: Uuid,
    pub user_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_cents: i32,
    pub stripe_invoice_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BillingLineItem {
    pub id: Uuid,
    pub billing_period_id: Uuid,
    pub database_id: Uuid,
    pub plan_template_id: Option<Uuid>,
    pub hours_used: f64,
    pub amount_cents: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CurrentUsageResponse {
    pub databases: Vec<DatabaseUsage>,
    pub total_estimated_cents: i32,
}

#[derive(Debug, Serialize)]
pub struct DatabaseUsage {
    pub database_id: Uuid,
    pub database_name: String,
    pub plan_name: Option<String>,
    pub hours_used: f64,
    pub estimated_cents: i32,
}
