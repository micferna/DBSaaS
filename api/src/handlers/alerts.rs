use axum::extract::{Path, State};
use crate::extract::Json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::models::alert::{AlertHistory, AlertRule, CreateAlertRuleRequest, UpdateAlertRuleRequest};
use crate::repository::{AlertRepository, DatabaseRepository};
use crate::services::alert::validate_webhook_url;
use crate::AppState;
use axum::Extension;

pub async fn list_alerts(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<AlertRule>>> {
    let rules = AlertRepository::list_by_user(&state.db, user.id).await?;
    Ok(Json(rules))
}

pub async fn create_alert(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateAlertRuleRequest>,
) -> AppResult<Json<AlertRule>> {
    let valid_types = ["db_down", "db_error", "backup_failed", "high_cpu", "high_memory"];
    if !valid_types.contains(&req.event_type.as_str()) {
        return Err(AppError::BadRequest(format!("Invalid event_type. Must be one of: {}", valid_types.join(", "))));
    }

    if req.webhook_url.is_none() && req.email.is_none() {
        return Err(AppError::BadRequest("At least one of webhook_url or email is required".to_string()));
    }

    // IDOR check: verify user owns the database
    if let Some(db_id) = req.database_id {
        let db = DatabaseRepository::find_by_id(&state.db, db_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
        if db.user_id != user.id {
            return Err(AppError::Forbidden);
        }
    }

    // Validate webhook URL (SSRF prevention)
    if let Some(ref url) = req.webhook_url {
        validate_webhook_url(url)?;
    }

    let rule = AlertRepository::create_rule(
        &state.db,
        user.id,
        req.database_id,
        &req.event_type,
        req.webhook_url.as_deref(),
        req.email.as_deref(),
        req.enabled.unwrap_or(true),
    )
    .await?;
    Ok(Json(rule))
}

pub async fn update_alert(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAlertRuleRequest>,
) -> AppResult<Json<AlertRule>> {
    let existing = AlertRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Alert rule not found".to_string()))?;

    if existing.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Validate webhook URL (SSRF prevention)
    if let Some(ref url) = req.webhook_url {
        validate_webhook_url(url)?;
    }

    let rule = AlertRepository::update_rule(
        &state.db,
        id,
        req.webhook_url.as_deref(),
        req.email.as_deref(),
        req.enabled,
    )
    .await?;
    Ok(Json(rule))
}

pub async fn delete_alert(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = AlertRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Alert rule not found".to_string()))?;

    if existing.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    AlertRepository::delete_rule(&state.db, id).await?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

pub async fn list_history(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<AlertHistory>>> {
    let history = AlertRepository::list_history_by_user(&state.db, user.id, 100).await?;
    Ok(Json(history))
}
