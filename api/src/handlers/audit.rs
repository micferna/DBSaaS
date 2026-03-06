use axum::extract::{Query, State};
use crate::extract::Json;

use crate::error::AppResult;
use crate::middleware::auth::AuthUser;
use crate::models::audit::{AuditLog, AuditLogQuery};
use crate::repository::AuditRepository;
use crate::AppState;
use axum::Extension;

pub async fn list_user_audit_logs(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Json<Vec<AuditLog>>> {
    let per_page = query.per_page.unwrap_or(50).min(100);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    let logs = AuditRepository::list_by_user(&state.db, user.id, per_page, offset).await?;
    Ok(Json(logs))
}

pub async fn list_admin_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Json<Vec<AuditLog>>> {
    let per_page = query.per_page.unwrap_or(50).min(100);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    let logs = AuditRepository::list_all(
        &state.db,
        per_page,
        offset,
        query.action.as_deref(),
        query.resource_type.as_deref(),
    )
    .await?;
    Ok(Json(logs))
}
