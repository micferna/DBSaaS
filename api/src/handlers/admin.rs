use axum::extract::{Path, State};
use crate::extract::Json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreateInvitationRequest, DbEvent, DbStatus, UserRole};
use crate::repository::{BillingRepository, DatabaseRepository, DockerServerRepository, InvitationRepository, PrivateNetworkRepository, UserRepository};
use crate::services::provisioner::ProvisionerService;
use crate::utils::crypto::generate_random_string;
use crate::utils::subdomain::subdomain_fqdn;
use crate::AppState;

pub async fn admin_stats(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let user_count = UserRepository::count(&state.db).await?;
    let db_count = DatabaseRepository::count_all(&state.db).await?;
    let reg_enabled = *state.registration_enabled.read().await;

    // Users created per day (last 30 days)
    let user_growth: Vec<(String, i64)> = sqlx::query_as(
        "SELECT TO_CHAR(created_at, 'YYYY-MM-DD') as day, COUNT(*) as cnt
         FROM users WHERE created_at > NOW() - INTERVAL '30 days'
         GROUP BY day ORDER BY day"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // DBs created per day (last 30 days)
    let db_growth: Vec<(String, i64)> = sqlx::query_as(
        "SELECT TO_CHAR(created_at, 'YYYY-MM-DD') as day, COUNT(*) as cnt
         FROM database_instances WHERE created_at > NOW() - INTERVAL '30 days'
         GROUP BY day ORDER BY day"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // DB status breakdown
    let status_breakdown: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status::TEXT, COUNT(*) FROM database_instances GROUP BY status"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // DB type breakdown
    let type_breakdown: Vec<(String, i64)> = sqlx::query_as(
        "SELECT db_type::TEXT, COUNT(*) FROM database_instances GROUP BY db_type"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Revenue per month
    let revenue_monthly: Vec<(String, i64)> = sqlx::query_as(
        "SELECT TO_CHAR(period_start, 'YYYY-MM') as month, SUM(total_cents) as total
         FROM billing_periods WHERE status = 'paid'
         GROUP BY month ORDER BY month"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "users": user_count,
        "databases": db_count,
        "registration_enabled": reg_enabled,
        "user_growth": user_growth.iter().map(|(d, c)| serde_json::json!({"date": d, "count": c})).collect::<Vec<_>>(),
        "db_growth": db_growth.iter().map(|(d, c)| serde_json::json!({"date": d, "count": c})).collect::<Vec<_>>(),
        "status_breakdown": status_breakdown.iter().map(|(s, c)| serde_json::json!({"status": s, "count": c})).collect::<Vec<_>>(),
        "type_breakdown": type_breakdown.iter().map(|(t, c)| serde_json::json!({"type": t, "count": c})).collect::<Vec<_>>(),
        "revenue_monthly": revenue_monthly.iter().map(|(m, t)| serde_json::json!({"month": m, "total": t})).collect::<Vec<_>>(),
    })))
}

/// PUT /api/admin/settings/registration — toggle registration
pub async fn toggle_registration(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let enabled = body["enabled"]
        .as_bool()
        .ok_or_else(|| AppError::BadRequest("enabled (bool) is required".to_string()))?;

    *state.registration_enabled.write().await = enabled;

    Ok(Json(serde_json::json!({
        "registration_enabled": enabled
    })))
}

pub async fn list_users(State(state): State<AppState>) -> AppResult<Json<Vec<serde_json::Value>>> {
    let users = UserRepository::list_all(&state.db).await?;

    // Get DB count per user
    let db_counts: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT user_id, COUNT(*) FROM database_instances GROUP BY user_id"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let result: Vec<serde_json::Value> = users
        .iter()
        .map(|u| {
            let dbs = db_counts.iter().find(|(id, _)| *id == u.id).map(|(_, c)| *c).unwrap_or(0);
            serde_json::json!({
                "id": u.id,
                "email": u.email,
                "role": u.role,
                "max_databases": u.max_databases,
                "database_count": dbs,
                "created_at": u.created_at,
            })
        })
        .collect();

    Ok(Json(result))
}

pub async fn update_user_role(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let role_str = body["role"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("role is required".to_string()))?;

    let role = match role_str {
        "admin" => UserRole::Admin,
        "user" => UserRole::User,
        _ => return Err(AppError::BadRequest("Invalid role".to_string())),
    };

    UserRepository::update_role(&state.db, user_id, &role).await?;

    Ok(Json(serde_json::json!({ "status": "updated" })))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let databases = DatabaseRepository::find_by_user(&state.db, user_id).await?;

    let provisioner = state.provisioner.clone();
    let traefik_service = state.traefik_service.clone();
    let port_pool = state.port_pool.clone();
    let pool = state.db.clone();

    for db in &databases {
        // Record billing stop event
        let _ = BillingRepository::record_usage_event(
            &state.db, db.id, "stop", db.user_id, &db.name, db.plan_template_id,
        ).await;

        DatabaseRepository::update_status(&state.db, db.id, &DbStatus::Deleting).await?;

        let target_docker = if let Some(server_id) = db.docker_server_id {
            DockerServerRepository::find_by_id(&state.db, server_id)
                .await
                .ok()
                .flatten()
                .and_then(|s| ProvisionerService::connect_to_server(&s).ok())
        } else {
            None
        };

        // Detach from private networks
        if let Some(ref cid) = db.container_id {
            if let Ok(networks) = PrivateNetworkRepository::find_networks_for_database(&state.db, db.id).await {
                for net in &networks {
                    if let Some(ref docker_net_id) = net.docker_network_id {
                        let _ = state.provisioner.detach_container_from_network(
                            target_docker.as_ref(), cid, docker_net_id,
                        ).await;
                    }
                }
            }
        }

        // Remove container + network, Traefik config, DB record, release port
        let db_id = db.id;
        let db_user_id = db.user_id;
        let port = db.port as u16;
        let container_id = db.container_id.clone();
        let network_id = db.network_id.clone();
        let prov = provisioner.clone();
        let traefik = traefik_service.clone();
        let pp = port_pool.clone();
        let p = pool.clone();
        let etx = state.event_tx.clone();

        // Notify SSE: deleting
        let _ = state.event_tx.send(DbEvent {
            user_id: db_user_id,
            database_id: db_id,
            event_type: "deleted".to_string(),
            status: None,
        });

        tokio::spawn(async move {
            if let (Some(cid), Some(nid)) = (&container_id, &network_id) {
                if let Err(e) = prov.remove_container(target_docker.as_ref(), cid, nid).await {
                    tracing::error!("Failed to remove container for db {db_id}: {e}");
                }
            }
            let _ = traefik.remove_config(&db_id.to_string());
            let _ = DatabaseRepository::delete(&p, db_id).await;
            pp.release(port);
            drop(etx);
        });
    }

    // Delete user's private networks
    if let Ok(networks) = PrivateNetworkRepository::find_by_user(&state.db, user_id).await {
        for net in &networks {
            if let Some(ref docker_net_id) = net.docker_network_id {
                let _ = state.provisioner.remove_private_network(None, docker_net_id).await;
            }
            let _ = PrivateNetworkRepository::delete(&state.db, net.id).await;
        }
    }

    UserRepository::delete(&state.db, user_id).await?;

    tracing::info!("Deleted user {user_id} and {} database(s)", databases.len());
    Ok(Json(serde_json::json!({ "status": "deleted", "databases_cleaned": databases.len() })))
}

pub async fn list_all_databases(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let dbs = DatabaseRepository::list_all(&state.db).await?;

    // Build lookup maps for users and servers
    let users = UserRepository::list_all(&state.db).await.unwrap_or_default();
    let user_map: std::collections::HashMap<Uuid, String> = users.into_iter().map(|u| (u.id, u.email)).collect();

    let servers = DockerServerRepository::list_all(&state.db).await.unwrap_or_default();
    let server_map: std::collections::HashMap<Uuid, String> = servers.into_iter().map(|s| (s.id, s.name)).collect();

    let result: Vec<serde_json::Value> = dbs
        .iter()
        .map(|d| {
            let user_email = user_map.get(&d.user_id).cloned().unwrap_or_default();
            let server_name = d.docker_server_id
                .and_then(|sid| server_map.get(&sid).cloned())
                .unwrap_or_else(|| "Local".to_string());

            serde_json::json!({
                "id": d.id,
                "user_id": d.user_id,
                "user_email": user_email,
                "name": d.name,
                "db_type": d.db_type,
                "status": d.status,
                "port": d.port,
                "cpu_limit": d.cpu_limit,
                "memory_limit_mb": d.memory_limit_mb,
                "plan_template_id": d.plan_template_id,
                "bundle_id": d.bundle_id,
                "docker_server_id": d.docker_server_id,
                "server_name": server_name,
                "subdomain": d.subdomain,
                "routing_mode": d.routing_mode,
                "created_at": d.created_at,
            })
        })
        .collect();

    Ok(Json(result))
}

pub async fn force_delete_database(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    let target_docker = if let Some(server_id) = inst.docker_server_id {
        DockerServerRepository::find_by_id(&state.db, server_id)
            .await
            .ok()
            .flatten()
            .and_then(|s| crate::services::provisioner::ProvisionerService::connect_to_server(&s).ok())
    } else {
        None
    };

    // Detach from private networks
    if let Some(ref cid) = inst.container_id {
        if let Ok(networks) = PrivateNetworkRepository::find_networks_for_database(&state.db, id).await {
            for net in &networks {
                if let Some(ref docker_net_id) = net.docker_network_id {
                    let _ = state.provisioner.detach_container_from_network(
                        target_docker.as_ref(), cid, docker_net_id,
                    ).await;
                }
            }
        }
    }

    if let (Some(cid), Some(nid)) = (&inst.container_id, &inst.network_id) {
        let _ = state.provisioner.remove_container(target_docker.as_ref(), cid, nid).await;
    }

    // Record stop event before deletion so billing can account for usage
    let _ = BillingRepository::record_usage_event(&state.db, id, "stop", inst.user_id, &inst.name, inst.plan_template_id).await;

    let _ = state.traefik_service.remove_config(&id.to_string());
    DatabaseRepository::delete(&state.db, id).await?;
    state.port_pool.release(inst.port as u16);

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

pub async fn create_invitation(
    State(state): State<AppState>,
    Json(req): Json<CreateInvitationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let code = generate_random_string(16);
    let max_uses = req.max_uses.unwrap_or(1);
    let expires_at = req.expires_in_hours.map(|h| {
        chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(h))
            .unwrap()
    });

    let admin_id = Uuid::nil();
    let invitation =
        InvitationRepository::create(&state.db, &code, admin_id, max_uses, expires_at).await?;

    Ok(Json(serde_json::json!({
        "id": invitation.id,
        "code": invitation.code,
        "max_uses": invitation.max_uses,
        "expires_at": invitation.expires_at,
    })))
}

pub async fn list_invitations(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let invitations = InvitationRepository::list_all(&state.db).await?;
    let result: Vec<serde_json::Value> = invitations
        .iter()
        .map(|i| {
            serde_json::json!({
                "id": i.id,
                "code": i.code,
                "max_uses": i.max_uses,
                "use_count": i.use_count,
                "expires_at": i.expires_at,
                "created_at": i.created_at,
            })
        })
        .collect();

    Ok(Json(result))
}

pub async fn delete_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    InvitationRepository::delete(&state.db, id).await?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

/// POST /api/admin/databases/{id}/migrate-sni
/// Migrate a legacy port-routed database to SNI routing.
pub async fn migrate_to_sni(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.routing_mode == "sni" {
        return Err(AppError::BadRequest("Database is already using SNI routing".to_string()));
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned yet".to_string()))?;

    // Resolve Docker client and server info for this instance
    let target_server = if let Some(server_id) = inst.docker_server_id {
        Some(
            DockerServerRepository::find_by_id(&state.db, server_id)
                .await?
                .ok_or_else(|| AppError::Internal("Docker server not found for instance".to_string()))?,
        )
    } else {
        None
    };
    let target_docker = if let Some(ref server) = target_server {
        Some(ProvisionerService::connect_to_server(server)?)
    } else {
        None
    };

    // Determine if remote
    let is_remote = target_server
        .as_ref()
        .map(|s| s.url != "local" && !s.url.starts_with("unix://") && !s.url.contains("localhost") && !s.url.contains("127.0.0.1"))
        .unwrap_or(false);

    // Generate TLS cert for the subdomain
    let fqdn = subdomain_fqdn(&inst.subdomain, &state.config.platform_domain);
    let cert = state.tls_service.generate_cert_for_subdomain(&fqdn)?;

    if !is_remote {
        // Local only: connect container to proxy network
        state
            .provisioner
            .connect_to_proxy_network(target_docker.as_ref(), container_id, "sb-proxy")
            .await?;
    }

    // Compute backend address
    let internal_port = match inst.db_type {
        crate::models::DbType::Postgresql => 5432,
        crate::models::DbType::Redis => 6379,
        crate::models::DbType::Mariadb => 3306,
    };
    let backend_address = if is_remote {
        if let Some(ref srv) = target_server {
            // Extract IP from server URL
            let without_scheme = srv.url
                .find("://")
                .map(|i| &srv.url[i + 3..])
                .unwrap_or(&srv.url);
            let host = if without_scheme.contains(':') {
                without_scheme.split(':').next().unwrap_or(without_scheme)
            } else {
                without_scheme
            };
            format!("{}:{}", host, inst.port)
        } else {
            format!("sb-{}:{}", id, internal_port)
        }
    } else {
        format!("sb-{}:{}", id, internal_port)
    };

    // Generate SNI Traefik config (replaces old port-based config)
    state.traefik_service.generate_sni_config(
        &id.to_string(),
        &inst.db_type,
        &fqdn,
        &cert.cert_pem,
        &cert.key_pem,
        &backend_address,
    )?;

    // Update routing mode
    DatabaseRepository::update_routing_mode(&state.db, id, "sni").await?;

    // Update TLS cert in DB
    DatabaseRepository::update_provisioned(
        &state.db,
        id,
        container_id,
        inst.network_id.as_deref().unwrap_or(""),
        Some(&cert.cert_pem),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "status": "migrated",
        "routing_mode": "sni",
        "subdomain": inst.subdomain,
        "fqdn": fqdn,
    })))
}

/// PUT /api/admin/settings/maintenance — toggle maintenance mode
pub async fn toggle_maintenance(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<serde_json::Value>> {
    let enabled = body["enabled"]
        .as_bool()
        .ok_or_else(|| AppError::BadRequest("enabled (bool) is required".to_string()))?;

    *state.maintenance_mode.write().await = enabled;

    // Persist to platform_settings
    let _ = sqlx::query(
        "INSERT INTO platform_settings (key, value, updated_at) VALUES ('maintenance_mode', $1, NOW()) ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()"
    )
    .bind(serde_json::json!(enabled))
    .execute(&state.db)
    .await;

    Ok(Json(serde_json::json!({ "maintenance_mode": enabled })))
}

/// GET /api/admin/health — system health overview
pub async fn system_health(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let servers = DockerServerRepository::list_all(&state.db).await?;
    let databases = DatabaseRepository::list_all(&state.db).await?;

    let mut server_health = Vec::new();
    for server in &servers {
        let docker = ProvisionerService::connect_to_server(server);
        let online = match &docker {
            Ok(d) => d.ping().await.is_ok(),
            Err(_) => false,
        };
        server_health.push(serde_json::json!({
            "id": server.id,
            "name": server.name,
            "url": server.url,
            "online": online,
            "server_type": server.server_type,
        }));
    }

    let mut db_statuses = Vec::new();
    for db in &databases {
        db_statuses.push(serde_json::json!({
            "id": db.id,
            "name": db.name,
            "db_type": db.db_type,
            "status": db.status,
            "docker_server_id": db.docker_server_id,
        }));
    }

    let maintenance = *state.maintenance_mode.read().await;

    Ok(Json(serde_json::json!({
        "servers": server_health,
        "databases": db_statuses,
        "maintenance_mode": maintenance,
    })))
}

/// GET /api/admin/users/{id}/resources — user's allocated resources
pub async fn user_resources(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let databases = DatabaseRepository::find_by_user(&state.db, user_id).await?;

    let total_cpu: f64 = databases.iter().map(|d| d.cpu_limit).sum();
    let total_memory_mb: i32 = databases.iter().map(|d| d.memory_limit_mb).sum();

    let db_list: Vec<serde_json::Value> = databases
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "name": d.name,
                "db_type": d.db_type,
                "status": d.status,
                "cpu_limit": d.cpu_limit,
                "memory_limit_mb": d.memory_limit_mb,
                "plan_template_id": d.plan_template_id,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "user_id": user_id,
        "total_cpu": total_cpu,
        "total_memory_mb": total_memory_mb,
        "databases": db_list,
    })))
}
