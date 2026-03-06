use axum::{
    extract::{Path, State},
    Extension,
};
use crate::extract::Json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::models::{
    BillingPeriod, CreatePlanTemplateRequest, CurrentUsageResponse, PlanTemplate,
    UpdatePlanTemplateRequest,
};
use crate::repository::BillingRepository;
use crate::services::billing::BillingService;
use crate::AppState;

// --- Public (no auth) routes ---

/// GET /api/public/plans — list active plans (no auth required, for landing page)
pub async fn public_list_plans(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PlanTemplate>>> {
    let templates = BillingRepository::list_active_templates(&state.db).await?;
    Ok(Json(templates))
}

// --- User routes ---

/// GET /api/plans — list active plans for clients
pub async fn list_plans(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<PlanTemplate>>> {
    let templates = BillingRepository::list_active_templates(&state.db).await?;
    Ok(Json(templates))
}

/// GET /api/billing/periods — user billing history
pub async fn billing_periods(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<BillingPeriod>>> {
    let periods = BillingRepository::get_user_billing_periods(&state.db, user.id).await?;
    Ok(Json(periods))
}

/// GET /api/billing/current — current month usage estimate
pub async fn billing_current(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<CurrentUsageResponse>> {
    let usage = BillingService::get_current_usage(&state.db, user.id).await?;
    Ok(Json(usage))
}

// --- Admin routes ---

/// GET /api/admin/plans — list all plans (including inactive)
pub async fn admin_list_plans(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PlanTemplate>>> {
    let templates = BillingRepository::list_plan_templates(&state.db).await?;
    Ok(Json(templates))
}

/// POST /api/admin/plans — create a plan template
pub async fn admin_create_plan(
    State(state): State<AppState>,
    Json(req): Json<CreatePlanTemplateRequest>,
) -> AppResult<Json<PlanTemplate>> {
    let template = BillingRepository::create_plan_template(&state.db, &req).await?;
    Ok(Json(template))
}

/// PUT /api/admin/plans/{id} — update a plan template
pub async fn admin_update_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlanTemplateRequest>,
) -> AppResult<Json<PlanTemplate>> {
    let template = BillingRepository::update_plan_template(&state.db, id, &req).await?;
    Ok(Json(template))
}

/// DELETE /api/admin/plans/{id} — deactivate a plan
pub async fn admin_delete_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    BillingRepository::delete_plan_template(&state.db, id).await?;
    Ok(Json(serde_json::json!({ "status": "deactivated" })))
}

/// GET /api/admin/billing/overview — global billing overview
pub async fn admin_billing_overview(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let periods = BillingRepository::get_all_billing_periods(&state.db).await?;
    let total_revenue: i32 = periods
        .iter()
        .filter(|p| p.status == "paid")
        .map(|p| p.total_cents)
        .sum();
    let pending_revenue: i32 = periods
        .iter()
        .filter(|p| p.status == "pending" || p.status == "invoiced")
        .map(|p| p.total_cents)
        .sum();

    Ok(Json(serde_json::json!({
        "total_revenue_cents": total_revenue,
        "pending_revenue_cents": pending_revenue,
        "total_periods": periods.len(),
        "periods": periods,
    })))
}

/// POST /api/admin/billing/generate — manually trigger billing
pub async fn admin_generate_billing(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let stripe_key = state.config.stripe_secret_key.as_deref();
    let count = BillingService::run_monthly_billing(&state.db, stripe_key).await?;
    Ok(Json(serde_json::json!({ "invoices_created": count })))
}

/// Verify Stripe webhook signature (HMAC-SHA256)
fn verify_stripe_signature(payload: &str, sig_header: &str, secret: &str) -> Result<(), AppError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    // Parse the Stripe-Signature header: t=TIMESTAMP,v1=SIGNATURE
    let mut timestamp = None;
    let mut signature = None;
    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t);
        } else if let Some(v) = part.strip_prefix("v1=") {
            signature = Some(v);
        }
    }

    let timestamp = timestamp
        .ok_or_else(|| AppError::BadRequest("Missing timestamp in Stripe signature".to_string()))?;
    let expected_sig = signature
        .ok_or_else(|| AppError::BadRequest("Missing v1 signature in Stripe header".to_string()))?;

    // Check timestamp tolerance (5 minutes)
    let ts: i64 = timestamp.parse().map_err(|_| {
        AppError::BadRequest("Invalid timestamp in Stripe signature".to_string())
    })?;
    let now = chrono::Utc::now().timestamp();
    if (now - ts).unsigned_abs() > 300 {
        return Err(AppError::BadRequest("Stripe webhook timestamp too old".to_string()));
    }

    // Compute expected signature: HMAC-SHA256(secret, "TIMESTAMP.PAYLOAD")
    let signed_payload = format!("{timestamp}.{payload}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| AppError::Internal(format!("HMAC key error: {e}")))?;
    mac.update(signed_payload.as_bytes());

    // Compare with hex-decoded expected signature
    let expected_bytes = hex::decode(expected_sig)
        .map_err(|_| AppError::BadRequest("Invalid hex in Stripe signature".to_string()))?;

    mac.verify_slice(&expected_bytes)
        .map_err(|_| AppError::BadRequest("Invalid Stripe webhook signature".to_string()))?;

    Ok(())
}

/// POST /api/stripe/webhook — handle Stripe webhooks
pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: String,
) -> AppResult<Json<serde_json::Value>> {
    // Verify Stripe signature — reject unsigned webhooks
    let webhook_secret = state.config.stripe_webhook_secret.as_ref()
        .ok_or_else(|| {
            tracing::error!("STRIPE_WEBHOOK_SECRET not set — rejecting webhook");
            AppError::Internal("Stripe webhook secret not configured".to_string())
        })?;

    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing Stripe-Signature header".to_string()))?;

    verify_stripe_signature(&body, sig_header, webhook_secret)?;

    // Parse the event from the body
    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {e}")))?;

    let event_type = event["type"]
        .as_str()
        .unwrap_or_default();

    let invoice_id = event["data"]["object"]["id"]
        .as_str()
        .unwrap_or_default();

    if !invoice_id.is_empty() {
        BillingService::handle_stripe_webhook(&state.db, event_type, invoice_id).await?;
    }

    Ok(Json(serde_json::json!({ "received": true })))
}
