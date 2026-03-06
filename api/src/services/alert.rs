use sqlx::PgPool;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::repository::{AlertRepository, DatabaseRepository, DockerServerRepository};
use crate::services::provisioner::ProvisionerService;

pub struct AlertService;

/// Validate a webhook URL to prevent SSRF attacks.
/// Only allows HTTPS URLs targeting non-internal IP addresses.
pub fn validate_webhook_url(url: &str) -> AppResult<()> {
    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::BadRequest("Invalid webhook URL".to_string()))?;

    // Only allow HTTPS
    if parsed.scheme() != "https" {
        return Err(AppError::BadRequest("Webhook URL must use HTTPS".to_string()));
    }

    let host = parsed.host_str()
        .ok_or_else(|| AppError::BadRequest("Webhook URL must have a host".to_string()))?;

    // Block obvious internal hostnames
    let blocked_hosts = ["localhost", "127.0.0.1", "0.0.0.0", "[::1]", "metadata.google.internal", "169.254.169.254"];
    if blocked_hosts.contains(&host) {
        return Err(AppError::BadRequest("Webhook URL cannot target internal addresses".to_string()));
    }

    // Resolve and check for private/internal IPs
    let port = parsed.port().unwrap_or(443);
    if let Ok(addrs) = format!("{host}:{port}").to_socket_addrs() {
        for addr in addrs {
            if is_internal_ip(addr.ip()) {
                return Err(AppError::BadRequest("Webhook URL resolves to an internal IP address".to_string()));
            }
        }
    }

    Ok(())
}

fn is_internal_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 169 && v4.octets()[1] == 254 // link-local
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // CGN
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified()
            // Most v6 internal ranges
        }
    }
}

impl AlertService {
    pub async fn check_container_health(
        pool: &PgPool,
        provisioner: &Arc<ProvisionerService>,
        config: &Arc<Config>,
    ) {
        let rules = match AlertRepository::find_enabled_rules(pool).await {
            Ok(r) => r,
            Err(_) => return,
        };

        for rule in &rules {
            let database_id = match rule.database_id {
                Some(id) => id,
                None => continue,
            };

            let db_inst = match DatabaseRepository::find_by_id(pool, database_id).await {
                Ok(Some(d)) => d,
                _ => continue,
            };

            let container_id = match db_inst.container_id.as_deref() {
                Some(cid) => cid,
                None => continue,
            };

            let target_docker = if let Some(server_id) = db_inst.docker_server_id {
                DockerServerRepository::find_by_id(pool, server_id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|s| ProvisionerService::connect_to_server(&s).ok())
            } else {
                None
            };

            match rule.event_type.as_str() {
                "db_down" => {
                    let running = provisioner
                        .is_container_running(target_docker.as_ref(), container_id)
                        .await
                        .unwrap_or(false);
                    if !running {
                        let msg = format!("Database '{}' is down", db_inst.name);
                        Self::fire_alert(pool, rule, &msg, config).await;
                    }
                }
                "high_cpu" | "high_memory" => {
                    if let Ok(Some(stats)) = provisioner
                        .get_container_stats_once(target_docker.as_ref(), container_id)
                        .await
                    {
                        if rule.event_type == "high_cpu" && stats.cpu_percent > 90.0 {
                            let msg = format!(
                                "Database '{}' CPU at {:.1}%",
                                db_inst.name, stats.cpu_percent
                            );
                            Self::fire_alert(pool, rule, &msg, config).await;
                        }
                        if rule.event_type == "high_memory" && stats.memory_percent > 90.0 {
                            let msg = format!(
                                "Database '{}' memory at {:.1}%",
                                db_inst.name, stats.memory_percent
                            );
                            Self::fire_alert(pool, rule, &msg, config).await;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    async fn fire_alert(
        pool: &PgPool,
        rule: &crate::models::alert::AlertRule,
        message: &str,
        config: &Arc<Config>,
    ) {
        // Record in history
        let _ = AlertRepository::insert_history(pool, rule.id, &rule.event_type, message).await;

        // Send webhook
        if let Some(ref url) = rule.webhook_url {
            Self::send_webhook(url, &rule.event_type, message).await;
        }

        // Send email
        if let Some(ref email) = rule.email {
            Self::send_email(config, email, &rule.event_type, message).await;
        }
    }

    async fn send_webhook(url: &str, event_type: &str, message: &str) {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "event_type": event_type,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        match client.post(url).json(&payload).send().await {
            Ok(resp) => {
                tracing::debug!("Webhook sent to {url}: status {}", resp.status());
            }
            Err(e) => {
                tracing::warn!("Failed to send webhook to {url}: {e}");
            }
        }
    }

    async fn send_email(config: &Arc<Config>, to: &str, event_type: &str, message: &str) {
        let smtp_host = match config.smtp_host.as_deref() {
            Some(h) if !h.is_empty() => h,
            _ => {
                tracing::debug!("SMTP not configured, skipping email alert");
                return;
            }
        };

        let from = config
            .smtp_from
            .as_deref()
            .unwrap_or("noreply@dbsaas.local");

        use lettre::{
            message::Message,
            transport::smtp::authentication::Credentials,
            AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
        };

        let email = match Message::builder()
            .from(from.parse().unwrap_or_else(|_| "noreply@dbsaas.local".parse().unwrap()))
            .to(match to.parse() {
                Ok(addr) => addr,
                Err(_) => return,
            })
            .subject(format!("DBSaaS Alert: {event_type}"))
            .body(message.to_string())
        {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to build email: {e}");
                return;
            }
        };

        let smtp_port = config.smtp_port.unwrap_or(587);

        let mailer_builder = match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Failed to create SMTP transport: {e}");
                return;
            }
        };

        let mailer = if let (Some(user), Some(pass)) = (&config.smtp_username, &config.smtp_password) {
            mailer_builder
                .port(smtp_port)
                .credentials(Credentials::new(user.clone(), pass.clone()))
                .build()
        } else {
            mailer_builder.port(smtp_port).build()
        };

        match mailer.send(email).await {
            Ok(_) => tracing::debug!("Alert email sent to {to}"),
            Err(e) => tracing::warn!("Failed to send alert email to {to}: {e}"),
        }
    }
}
