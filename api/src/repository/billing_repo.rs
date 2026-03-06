use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{
    BillingLineItem, BillingPeriod, CreatePlanTemplateRequest, PlanTemplate,
    UpdatePlanTemplateRequest, UsageEvent,
};

pub struct BillingRepository;

impl BillingRepository {
    // --- Plan Templates ---

    pub async fn create_plan_template(
        pool: &PgPool,
        req: &CreatePlanTemplateRequest,
    ) -> AppResult<PlanTemplate> {
        let template = sqlx::query_as::<_, PlanTemplate>(
            r#"INSERT INTO plan_templates (name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at"#,
        )
        .bind(&req.name)
        .bind(&req.db_type)
        .bind(req.cpu_limit)
        .bind(req.memory_limit_mb)
        .bind(req.monthly_price_cents)
        .bind(req.hourly_price_cents)
        .bind(req.is_bundle)
        .bind(req.active)
        .fetch_one(pool)
        .await?;
        Ok(template)
    }

    pub async fn list_plan_templates(pool: &PgPool) -> AppResult<Vec<PlanTemplate>> {
        let templates = sqlx::query_as::<_, PlanTemplate>(
            "SELECT id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at FROM plan_templates ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(templates)
    }

    pub async fn list_active_templates(pool: &PgPool) -> AppResult<Vec<PlanTemplate>> {
        let templates = sqlx::query_as::<_, PlanTemplate>(
            "SELECT id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at FROM plan_templates WHERE active = true ORDER BY monthly_price_cents",
        )
        .fetch_all(pool)
        .await?;
        Ok(templates)
    }

    pub async fn get_plan_template(
        pool: &PgPool,
        id: Uuid,
    ) -> AppResult<Option<PlanTemplate>> {
        let template = sqlx::query_as::<_, PlanTemplate>(
            "SELECT id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at FROM plan_templates WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(template)
    }

    pub async fn update_plan_template(
        pool: &PgPool,
        id: Uuid,
        req: &UpdatePlanTemplateRequest,
    ) -> AppResult<PlanTemplate> {
        let current = sqlx::query_as::<_, PlanTemplate>(
            "SELECT id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at FROM plan_templates WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        let name = req.name.as_deref().unwrap_or(&current.name);
        let cpu_limit = req.cpu_limit.unwrap_or(current.cpu_limit);
        let memory_limit_mb = req.memory_limit_mb.unwrap_or(current.memory_limit_mb);
        let monthly_price_cents = req.monthly_price_cents.unwrap_or(current.monthly_price_cents);
        let hourly_price_cents = req.hourly_price_cents.unwrap_or(current.hourly_price_cents);
        let is_bundle = req.is_bundle.unwrap_or(current.is_bundle);
        let active = req.active.unwrap_or(current.active);

        let updated = sqlx::query_as::<_, PlanTemplate>(
            r#"UPDATE plan_templates SET name = $1, cpu_limit = $2, memory_limit_mb = $3, monthly_price_cents = $4, hourly_price_cents = $5, is_bundle = $6, active = $7
               WHERE id = $8
               RETURNING id, name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active, created_at"#,
        )
        .bind(name)
        .bind(cpu_limit)
        .bind(memory_limit_mb)
        .bind(monthly_price_cents)
        .bind(hourly_price_cents)
        .bind(is_bundle)
        .bind(active)
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(updated)
    }

    pub async fn delete_plan_template(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE plan_templates SET active = false WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // --- Usage Events ---

    pub async fn record_usage_event(
        pool: &PgPool,
        database_id: Uuid,
        event_type: &str,
        user_id: Uuid,
        database_name: &str,
        plan_template_id: Option<Uuid>,
    ) -> AppResult<UsageEvent> {
        let event = sqlx::query_as::<_, UsageEvent>(
            r#"INSERT INTO usage_events (database_id, event_type, user_id, database_name, plan_template_id)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, database_id, event_type, recorded_at, user_id, database_name, plan_template_id"#,
        )
        .bind(database_id)
        .bind(event_type)
        .bind(user_id)
        .bind(database_name)
        .bind(plan_template_id)
        .fetch_one(pool)
        .await?;
        Ok(event)
    }

    pub async fn get_usage_events(
        pool: &PgPool,
        database_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<UsageEvent>> {
        let events = sqlx::query_as::<_, UsageEvent>(
            r#"SELECT id, database_id, event_type, recorded_at, user_id, database_name, plan_template_id
               FROM usage_events WHERE database_id = $1 AND recorded_at >= $2 AND recorded_at <= $3
               ORDER BY recorded_at"#,
        )
        .bind(database_id)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?;
        Ok(events)
    }

    /// Get all distinct database_ids that had usage events for a user in a period.
    /// Returns (database_id, database_name, plan_template_id) tuples.
    pub async fn get_databases_with_usage_in_period(
        pool: &PgPool,
        user_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<(Uuid, Option<String>, Option<Uuid>)>> {
        let rows: Vec<(Uuid, Option<String>, Option<Uuid>)> = sqlx::query_as(
            r#"SELECT DISTINCT database_id, database_name, plan_template_id
               FROM usage_events
               WHERE user_id = $1 AND recorded_at >= $2 AND recorded_at <= $3"#,
        )
        .bind(user_id)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    // --- Billing Periods ---

    pub async fn create_billing_period(
        pool: &PgPool,
        user_id: Uuid,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        total_cents: i32,
    ) -> AppResult<BillingPeriod> {
        let period = sqlx::query_as::<_, BillingPeriod>(
            r#"INSERT INTO billing_periods (user_id, period_start, period_end, total_cents)
               VALUES ($1, $2, $3, $4)
               RETURNING id, user_id, period_start, period_end, total_cents, stripe_invoice_id, status, created_at"#,
        )
        .bind(user_id)
        .bind(period_start)
        .bind(period_end)
        .bind(total_cents)
        .fetch_one(pool)
        .await?;
        Ok(period)
    }

    pub async fn add_line_item(
        pool: &PgPool,
        billing_period_id: Uuid,
        database_id: Uuid,
        plan_template_id: Option<Uuid>,
        hours_used: f64,
        amount_cents: i32,
    ) -> AppResult<BillingLineItem> {
        let item = sqlx::query_as::<_, BillingLineItem>(
            r#"INSERT INTO billing_line_items (billing_period_id, database_id, plan_template_id, hours_used, amount_cents)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, billing_period_id, database_id, plan_template_id, hours_used, amount_cents, created_at"#,
        )
        .bind(billing_period_id)
        .bind(database_id)
        .bind(plan_template_id)
        .bind(hours_used)
        .bind(amount_cents)
        .fetch_one(pool)
        .await?;
        Ok(item)
    }

    pub async fn update_period_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        stripe_invoice_id: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE billing_periods SET status = $1, stripe_invoice_id = COALESCE($2, stripe_invoice_id) WHERE id = $3",
        )
        .bind(status)
        .bind(stripe_invoice_id)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_billing_periods(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<BillingPeriod>> {
        let periods = sqlx::query_as::<_, BillingPeriod>(
            "SELECT id, user_id, period_start, period_end, total_cents, stripe_invoice_id, status, created_at FROM billing_periods WHERE user_id = $1 ORDER BY period_start DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(periods)
    }

    pub async fn get_all_billing_periods(pool: &PgPool) -> AppResult<Vec<BillingPeriod>> {
        let periods = sqlx::query_as::<_, BillingPeriod>(
            "SELECT id, user_id, period_start, period_end, total_cents, stripe_invoice_id, status, created_at FROM billing_periods ORDER BY period_start DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(periods)
    }

    pub async fn get_line_items_for_period(
        pool: &PgPool,
        billing_period_id: Uuid,
    ) -> AppResult<Vec<BillingLineItem>> {
        let items = sqlx::query_as::<_, BillingLineItem>(
            "SELECT id, billing_period_id, database_id, plan_template_id, hours_used, amount_cents, created_at FROM billing_line_items WHERE billing_period_id = $1 ORDER BY created_at",
        )
        .bind(billing_period_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }
}
