use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::audit::AuditLog;

pub struct AuditRepository;

impl AuditRepository {
    pub async fn insert(
        pool: &PgPool,
        user_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Option<Uuid>,
        details: Option<serde_json::Value>,
        ip_address: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details, ip_address) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .bind(ip_address)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Fire-and-forget audit log via tokio::spawn
    pub fn log_async(
        pool: PgPool,
        user_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Option<Uuid>,
        details: Option<serde_json::Value>,
        ip_address: Option<String>,
    ) {
        let action = action.to_string();
        let resource_type = resource_type.to_string();
        tokio::spawn(async move {
            if let Err(e) = Self::insert(
                &pool,
                user_id,
                &action,
                &resource_type,
                resource_id,
                details,
                ip_address.as_deref(),
            )
            .await
            {
                tracing::warn!("Failed to write audit log: {e}");
            }
        });
    }

    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT id, user_id, action, resource_type, resource_id, details, ip_address, created_at FROM audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(logs)
    }

    pub async fn list_all(
        pool: &PgPool,
        limit: i64,
        offset: i64,
        action_filter: Option<&str>,
        resource_type_filter: Option<&str>,
    ) -> AppResult<Vec<AuditLog>> {
        let mut query = String::from(
            "SELECT id, user_id, action, resource_type, resource_id, details, ip_address, created_at FROM audit_logs WHERE 1=1"
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<String> = Vec::new();

        if let Some(a) = action_filter {
            param_idx += 1;
            query.push_str(&format!(" AND action = ${param_idx}"));
            binds.push(a.to_string());
        }
        if let Some(r) = resource_type_filter {
            param_idx += 1;
            query.push_str(&format!(" AND resource_type = ${param_idx}"));
            binds.push(r.to_string());
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx + 1, param_idx + 2));

        // Build query dynamically
        let mut q = sqlx::query_as::<_, AuditLog>(&query);
        for b in &binds {
            q = q.bind(b);
        }
        q = q.bind(limit).bind(offset);

        let logs = q.fetch_all(pool).await?;
        Ok(logs)
    }
}
