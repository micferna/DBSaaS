use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::alert::{AlertHistory, AlertRule};

pub struct AlertRepository;

impl AlertRepository {
    pub async fn create_rule(
        pool: &PgPool,
        user_id: Uuid,
        database_id: Option<Uuid>,
        event_type: &str,
        webhook_url: Option<&str>,
        email: Option<&str>,
        enabled: bool,
    ) -> AppResult<AlertRule> {
        let rule = sqlx::query_as::<_, AlertRule>(
            "INSERT INTO alert_rules (user_id, database_id, event_type, webhook_url, email, enabled) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, user_id, database_id, event_type, webhook_url, email, enabled, created_at"
        )
        .bind(user_id)
        .bind(database_id)
        .bind(event_type)
        .bind(webhook_url)
        .bind(email)
        .bind(enabled)
        .fetch_one(pool)
        .await?;
        Ok(rule)
    }

    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<AlertRule>> {
        let rules = sqlx::query_as::<_, AlertRule>(
            "SELECT id, user_id, database_id, event_type, webhook_url, email, enabled, created_at FROM alert_rules WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rules)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<AlertRule>> {
        let rule = sqlx::query_as::<_, AlertRule>(
            "SELECT id, user_id, database_id, event_type, webhook_url, email, enabled, created_at FROM alert_rules WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(rule)
    }

    pub async fn update_rule(
        pool: &PgPool,
        id: Uuid,
        webhook_url: Option<&str>,
        email: Option<&str>,
        enabled: Option<bool>,
    ) -> AppResult<AlertRule> {
        let rule = sqlx::query_as::<_, AlertRule>(
            "UPDATE alert_rules SET webhook_url = COALESCE($2, webhook_url), email = COALESCE($3, email), enabled = COALESCE($4, enabled) WHERE id = $1 RETURNING id, user_id, database_id, event_type, webhook_url, email, enabled, created_at"
        )
        .bind(id)
        .bind(webhook_url)
        .bind(email)
        .bind(enabled)
        .fetch_one(pool)
        .await?;
        Ok(rule)
    }

    pub async fn delete_rule(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM alert_rules WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn insert_history(
        pool: &PgPool,
        alert_rule_id: Uuid,
        event_type: &str,
        message: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO alert_history (alert_rule_id, event_type, message) VALUES ($1, $2, $3)"
        )
        .bind(alert_rule_id)
        .bind(event_type)
        .bind(message)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_history_by_user(pool: &PgPool, user_id: Uuid, limit: i64) -> AppResult<Vec<AlertHistory>> {
        let history = sqlx::query_as::<_, AlertHistory>(
            "SELECT ah.id, ah.alert_rule_id, ah.event_type, ah.message, ah.sent_at FROM alert_history ah JOIN alert_rules ar ON ah.alert_rule_id = ar.id WHERE ar.user_id = $1 ORDER BY ah.sent_at DESC LIMIT $2"
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(history)
    }

    pub async fn find_enabled_rules(pool: &PgPool) -> AppResult<Vec<AlertRule>> {
        let rules = sqlx::query_as::<_, AlertRule>(
            "SELECT id, user_id, database_id, event_type, webhook_url, email, enabled, created_at FROM alert_rules WHERE enabled = true"
        )
        .fetch_all(pool)
        .await?;
        Ok(rules)
    }
}
