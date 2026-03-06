use chrono::{DateTime, Datelike, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{BillingPeriod, CurrentUsageResponse, DatabaseUsage, PlanTemplate};
use crate::repository::{BillingRepository, DatabaseRepository};

pub struct BillingService;

impl BillingService {
    /// Calculate total running hours for a database within a period
    pub async fn calculate_usage_hours(
        pool: &PgPool,
        database_id: Uuid,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> AppResult<f64> {
        let events =
            BillingRepository::get_usage_events(pool, database_id, period_start, period_end)
                .await?;

        let mut total_seconds: f64 = 0.0;
        let mut last_start: Option<DateTime<Utc>> = None;

        // If no events but DB existed before period, assume it was running from period_start
        if events.is_empty() {
            let safe_min = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
            let prior_events = BillingRepository::get_usage_events(
                pool,
                database_id,
                safe_min,
                period_start,
            )
            .await?;

            if let Some(last_event) = prior_events.last() {
                if last_event.event_type == "start" {
                    let duration = period_end - period_start;
                    return Ok(duration.num_seconds() as f64 / 3600.0);
                }
            }
            return Ok(0.0);
        }

        // If first event is "stop", the DB was running from period_start
        if events.first().map(|e| e.event_type.as_str()) == Some("stop") {
            last_start = Some(period_start);
        }

        for event in &events {
            match event.event_type.as_str() {
                "start" => {
                    last_start = Some(event.recorded_at);
                }
                "stop" => {
                    if let Some(start) = last_start {
                        let duration = event.recorded_at - start;
                        total_seconds += duration.num_seconds() as f64;
                        last_start = None;
                    }
                }
                _ => {}
            }
        }

        // If still running at period_end
        if let Some(start) = last_start {
            let duration = period_end - start;
            total_seconds += duration.num_seconds() as f64;
        }

        Ok(total_seconds / 3600.0)
    }

    /// Calculate amount: min(hours * hourly_price, monthly_price)
    pub fn calculate_amount(hours: f64, template: &PlanTemplate) -> i32 {
        let hourly_total = (hours * template.hourly_price_cents as f64).ceil() as i32;
        std::cmp::min(hourly_total, template.monthly_price_cents)
    }

    /// Generate monthly invoice for a user
    pub async fn generate_monthly_invoice(
        pool: &PgPool,
        user_id: Uuid,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> AppResult<Option<BillingPeriod>> {
        // Anti-duplicate: check if a billing period already exists for this user+period
        let existing: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM billing_periods WHERE user_id = $1 AND period_start = $2",
        )
        .bind(user_id)
        .bind(period_start)
        .fetch_one(pool)
        .await?;

        if existing.0 > 0 {
            return Ok(None);
        }

        // Get ALL databases that had usage in this period (via usage_events), including deleted ones
        let usage_entries =
            BillingRepository::get_databases_with_usage_in_period(pool, user_id, period_start, period_end)
                .await?;

        if usage_entries.is_empty() {
            return Ok(None);
        }

        let mut total_cents = 0i32;
        let mut line_items: Vec<(Uuid, Option<Uuid>, f64, i32)> = Vec::new();

        for (database_id, _db_name, plan_tid) in &usage_entries {
            let hours =
                Self::calculate_usage_hours(pool, *database_id, period_start, period_end).await?;

            if hours <= 0.0 {
                continue;
            }

            let amount = if let Some(template_id) = plan_tid {
                if let Some(template) =
                    BillingRepository::get_plan_template(pool, *template_id).await?
                {
                    Self::calculate_amount(hours, &template)
                } else {
                    0
                }
            } else {
                0
            };

            total_cents += amount;
            line_items.push((*database_id, *plan_tid, hours, amount));
        }

        if total_cents == 0 && line_items.is_empty() {
            return Ok(None);
        }

        let period = BillingRepository::create_billing_period(
            pool,
            user_id,
            period_start,
            period_end,
            total_cents,
        )
        .await?;

        for (db_id, template_id, hours, amount) in line_items {
            BillingRepository::add_line_item(pool, period.id, db_id, template_id, hours, amount)
                .await?;
        }

        Ok(Some(period))
    }

    /// Get current month usage estimate for a user
    pub async fn get_current_usage(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<CurrentUsageResponse> {
        let now = Utc::now();
        let period_start = Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .unwrap();

        let mut db_usages = Vec::new();
        let mut total = 0i32;
        let mut seen_ids = std::collections::HashSet::new();

        // 1. Existing databases
        let databases = DatabaseRepository::find_by_user(pool, user_id).await?;
        for db in &databases {
            seen_ids.insert(db.id);
            let hours = Self::calculate_usage_hours(pool, db.id, period_start, now).await?;

            let (plan_name, estimated) = if let Some(template_id) = db.plan_template_id {
                if let Some(template) =
                    BillingRepository::get_plan_template(pool, template_id).await?
                {
                    let amount = Self::calculate_amount(hours, &template);
                    (Some(template.name.clone()), amount)
                } else {
                    (None, 0)
                }
            } else {
                (None, 0)
            };

            total += estimated;
            db_usages.push(DatabaseUsage {
                database_id: db.id,
                database_name: db.name.clone(),
                plan_name,
                hours_used: hours,
                estimated_cents: estimated,
            });
        }

        // 2. Deleted databases that had usage this period (from usage_events)
        let usage_entries =
            BillingRepository::get_databases_with_usage_in_period(pool, user_id, period_start, now)
                .await?;

        for (database_id, db_name, plan_tid) in &usage_entries {
            if seen_ids.contains(database_id) {
                continue; // already counted above
            }

            let hours = Self::calculate_usage_hours(pool, *database_id, period_start, now).await?;

            let (plan_name, estimated) = if let Some(template_id) = plan_tid {
                if let Some(template) =
                    BillingRepository::get_plan_template(pool, *template_id).await?
                {
                    let amount = Self::calculate_amount(hours, &template);
                    (Some(template.name.clone()), amount)
                } else {
                    (None, 0)
                }
            } else {
                (None, 0)
            };

            total += estimated;
            db_usages.push(DatabaseUsage {
                database_id: *database_id,
                database_name: db_name.clone().unwrap_or_else(|| "(deleted)".to_string()),
                plan_name,
                hours_used: hours,
                estimated_cents: estimated,
            });
        }

        Ok(CurrentUsageResponse {
            databases: db_usages,
            total_estimated_cents: total,
        })
    }

    /// Create a Stripe invoice for a billing period (direct HTTP API calls)
    pub async fn create_stripe_invoice(
        pool: &PgPool,
        billing_period_id: Uuid,
        stripe_secret_key: &str,
    ) -> AppResult<String> {
        let period = sqlx::query_as::<_, BillingPeriod>(
            "SELECT id, user_id, period_start, period_end, total_cents, stripe_invoice_id, status, created_at FROM billing_periods WHERE id = $1",
        )
        .bind(billing_period_id)
        .fetch_one(pool)
        .await?;

        if period.total_cents == 0 {
            return Ok("no_charge".to_string());
        }

        // Get user's stripe customer id
        let (stripe_customer_id,): (Option<String>,) = sqlx::query_as(
            "SELECT stripe_customer_id FROM users WHERE id = $1",
        )
        .bind(period.user_id)
        .fetch_one(pool)
        .await?;

        let customer_id = stripe_customer_id.ok_or_else(|| {
            AppError::BadRequest("User has no Stripe customer ID".to_string())
        })?;

        let http = reqwest::Client::new();
        let line_items =
            BillingRepository::get_line_items_for_period(pool, billing_period_id).await?;

        // Create invoice items (they attach to the customer's upcoming invoice)
        for item in &line_items {
            let desc = format!("{:.1}h usage", item.hours_used);
            let resp = http
                .post("https://api.stripe.com/v1/invoiceitems")
                .bearer_auth(stripe_secret_key)
                .form(&[
                    ("customer", customer_id.as_str()),
                    ("amount", &item.amount_cents.to_string()),
                    ("currency", "eur"),
                    ("description", &desc),
                ])
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Stripe request error: {e}")))?;

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(AppError::Internal(format!("Stripe invoiceitem error: {body}")));
            }
        }

        // Create and auto-finalize the invoice
        let resp = http
            .post("https://api.stripe.com/v1/invoices")
            .bearer_auth(stripe_secret_key)
            .form(&[
                ("customer", customer_id.as_str()),
                ("auto_advance", "true"),
            ])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe request error: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe invoice error: {body}")));
        }

        let invoice: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe JSON error: {e}")))?;

        let invoice_id = invoice["id"]
            .as_str()
            .ok_or_else(|| AppError::Internal("Stripe invoice missing id".to_string()))?
            .to_string();

        // Update billing period
        BillingRepository::update_period_status(pool, billing_period_id, "invoiced", Some(&invoice_id))
            .await?;

        Ok(invoice_id)
    }

    /// Handle Stripe webhook events
    pub async fn handle_stripe_webhook(
        pool: &PgPool,
        event_type: &str,
        invoice_id: &str,
    ) -> AppResult<()> {
        let new_status = match event_type {
            "invoice.paid" => "paid",
            "invoice.payment_failed" => "failed",
            _ => return Ok(()),
        };

        sqlx::query(
            "UPDATE billing_periods SET status = $1 WHERE stripe_invoice_id = $2",
        )
        .bind(new_status)
        .bind(invoice_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Run the monthly billing generation for all users
    pub async fn run_monthly_billing(pool: &PgPool, stripe_key: Option<&str>) -> AppResult<u32> {
        let now = Utc::now();
        // Previous month
        let (year, month) = if now.month() == 1 {
            (now.year() - 1, 12u32)
        } else {
            (now.year(), now.month() - 1)
        };

        let period_start = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).unwrap();
        let period_end = Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .unwrap();

        // Get all users
        let users: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users")
            .fetch_all(pool)
            .await?;

        let mut invoices_created = 0u32;
        for (user_id,) in users {
            match Self::generate_monthly_invoice(pool, user_id, period_start, period_end).await {
                Ok(Some(period)) => {
                    invoices_created += 1;
                    if let Some(key) = stripe_key {
                        if let Err(e) = Self::create_stripe_invoice(pool, period.id, key).await {
                            tracing::error!("Failed to create Stripe invoice for user {user_id}: {e}");
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!("Failed to generate invoice for user {user_id}: {e}");
                }
            }
        }

        Ok(invoices_created)
    }
}
