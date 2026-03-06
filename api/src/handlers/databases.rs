use axum::{
    extract::{Path, Query, State},
    Extension,
    response::sse::{Event, Sse},
};
use crate::extract::Json;
use futures_util::Stream;
use uuid::Uuid;
use validator::Validate;
use std::convert::Infallible;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::{AuthUser, Claims};
use crate::models::{
    BackupRecord, BundleResponse, ContainerAction, ContainerActionRequest, CreateBundleRequest,
    CreateDatabaseRequest, CreateDatabaseUserRequest, DatabaseInstance, DatabaseResponse, DatabaseUserListItem,
    DatabaseUserResponse, DbEvent, DbStatus, DbType,
};
use crate::repository::{BillingRepository, DatabaseRepository, DockerServerRepository, PrivateNetworkRepository};
use crate::services::provisioner::ProvisionerService;
use crate::utils::crypto::{decrypt_string, encrypt_string, generate_random_string};
use crate::utils::subdomain::{generate_subdomain, subdomain_fqdn};
use crate::AppState;

/// Resolve the Docker client for an existing database instance.
/// If the instance has a docker_server_id, connect to that server.
/// Otherwise, returns None (fallback to local).
async fn resolve_docker_for_instance(
    state: &AppState,
    inst: &DatabaseInstance,
) -> AppResult<Option<bollard::Docker>> {
    if let Some(server_id) = inst.docker_server_id {
        let server = DockerServerRepository::find_by_id(&state.db, server_id)
            .await?
            .ok_or_else(|| AppError::Internal("Docker server not found for instance".to_string()))?;
        let docker = ProvisionerService::connect_to_server(&server)?;
        Ok(Some(docker))
    } else {
        Ok(None)
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SseQuery {
    pub token: String,
}

pub async fn database_events(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Validate JWT from query param (EventSource can't set headers)
    let claims = jsonwebtoken::decode::<Claims>(
        &query.token,
        &jsonwebtoken::DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    let user_id = claims.sub;
    let mut rx = state.event_tx.subscribe();

    let stream = async_stream::stream! {
        // Send initial keepalive
        yield Ok(Event::default().comment("connected"));

        loop {
            match rx.recv().await {
                Ok(event) => {
                    if event.user_id == user_id {
                        if let Ok(data) = serde_json::to_string(&event) {
                            yield Ok(Event::default()
                                .event(&event.event_type)
                                .data(data));
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE client lagged by {n} messages");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ))
}

pub async fn create_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateDatabaseRequest>,
) -> AppResult<Json<DatabaseResponse>> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Validate ssl_mode
    let ssl_mode = match req.ssl_mode.as_str() {
        "verify-ca" | "require" => req.ssl_mode.as_str(),
        _ => return Err(AppError::BadRequest("ssl_mode must be 'require' or 'verify-ca'".to_string())),
    };

    // Check limits
    let count = DatabaseRepository::count_by_user(&state.db, user.id).await?;
    let max = state.config.max_databases_per_user as i64;
    if count >= max {
        return Err(AppError::BadRequest(format!(
            "Database limit reached ({max})"
        )));
    }

    // Resolve plan template if provided
    let (cpu_limit, memory_limit_mb, plan_template_id) = if let Some(template_id) = req.plan_template_id {
        let template = BillingRepository::get_plan_template(&state.db, template_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Plan template not found".to_string()))?;
        if !template.active {
            return Err(AppError::BadRequest("Plan template is not active".to_string()));
        }
        (template.cpu_limit, template.memory_limit_mb, Some(template_id))
    } else {
        (req.cpu_limit.unwrap_or(0.5), req.memory_limit_mb.unwrap_or(256), None)
    };
    // SNI mode: TLS always enabled at Traefik level
    let tls_mode = ssl_mode;

    // Select Docker server: explicit choice or auto-select
    let target_server = if let Some(server_id) = req.server_id {
        let server = DockerServerRepository::find_active_client_by_id(&state.db, server_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Selected server not found or not available".to_string()))?;
        let count = DockerServerRepository::count_containers_on_server(&state.db, server.id).await?;
        if count >= server.max_containers as i64 {
            return Err(AppError::BadRequest("Selected server is at capacity".to_string()));
        }
        Some(server)
    } else {
        DockerServerRepository::select_best_client_server(&state.db).await?
    };
    let docker_server_id = target_server.as_ref().map(|s| s.id);
    let target_server_clone = target_server.clone();

    // Generate credentials
    let username = format!("u_{}", generate_random_string(12));
    let password = generate_random_string(32);
    let db_name = match &req.db_type {
        DbType::Postgresql | DbType::Mariadb => Some(req.name.clone()),
        DbType::Redis => None,
    };

    let password_encrypted = encrypt_string(&password, &state.config.encryption_key)?;

    // Allocate a port for legacy fallback but SNI mode uses standard ports
    let port = state.port_pool.allocate()? as i32;

    // Generate subdomain
    let temp_id = Uuid::new_v4();

    // Create DB record with SNI routing mode
    let db_instance = DatabaseRepository::create(
        &state.db,
        user.id,
        &req.name,
        &req.db_type,
        &state.config.platform_domain,
        port,
        &username,
        &password_encrypted,
        db_name.as_deref(),
        cpu_limit,
        memory_limit_mb,
        None,
        tls_mode,
        plan_template_id,
        docker_server_id,
        &generate_subdomain(&req.name, temp_id),
        "sni",
    )
    .await;

    // If subdomain conflict, regenerate with actual ID
    let db_instance = match db_instance {
        Ok(inst) => inst,
        Err(_) => {
            // Retry — the RETURNING gives us the real ID
            let fallback_sub = generate_subdomain(&req.name, Uuid::new_v4());
            DatabaseRepository::create(
                &state.db,
                user.id,
                &req.name,
                &req.db_type,
                &state.config.platform_domain,
                port,
                &username,
                &password_encrypted,
                db_name.as_deref(),
                cpu_limit,
                memory_limit_mb,
                None,
                tls_mode,
                plan_template_id,
                docker_server_id,
                &fallback_sub,
                "sni",
            )
            .await?
        }
    };

    let db_id = db_instance.id;
    let subdomain = db_instance.subdomain.clone();
    let fqdn = subdomain_fqdn(&subdomain, &state.config.platform_domain);
    let billing_user_id = user.id;
    let billing_db_name = req.name.clone();
    let billing_plan_id = plan_template_id;

    // Provision async
    let provisioner = state.provisioner.clone();
    let pool = state.db.clone();
    let tls_service = state.tls_service.clone();
    let traefik_service = state.traefik_service.clone();
    let db_type = req.db_type.clone();
    let port_pool = state.port_pool.clone();
    let fqdn_clone = fqdn.clone();

    let username_clone = username.clone();
    let password_clone = password.clone();
    let db_name_clone = db_name.clone();
    let event_tx = state.event_tx.clone();

    tokio::spawn(async move {
        let result = async {
            let target_docker = if let Some(ref server) = target_server_clone {
                Some(ProvisionerService::connect_to_server(server)?)
            } else {
                None
            };

            // Determine if this is a remote server
            let is_remote = target_server_clone
                .as_ref()
                .map(|s| is_remote_server(&s.url))
                .unwrap_or(false);

            // For remote servers, use the allocated port as exposed port
            let exposed_port = if is_remote { Some(port as u16) } else { None };

            // Extract host IP for remote servers (bind on private IP instead of 0.0.0.0)
            let host_ip_owned = target_server_clone
                .as_ref()
                .and_then(|s| extract_server_ip(&s.url));

            // Generate TLS cert for the subdomain FQDN
            let cert = tls_service.generate_cert_for_subdomain(&fqdn_clone)?;

            // Create container in SNI mode
            let provision_result = provisioner
                .create_database_sni(
                    target_docker.as_ref(),
                    db_id,
                    &db_type,
                    &username_clone,
                    &password_clone,
                    db_name_clone.as_deref().unwrap_or("default"),
                    cpu_limit,
                    memory_limit_mb as i64,
                    is_remote,
                    exposed_port,
                    host_ip_owned.as_deref(),
                )
                .await?;

            // Wait for container to become healthy before marking as provisioned
            let fallback = provisioner.fallback_docker();
            let docker_ref = target_docker.as_ref().unwrap_or(fallback);
            ProvisionerService::wait_for_healthy(docker_ref, &provision_result.container_id, 90).await?;

            // Compute backend address for Traefik
            let backend_address = compute_backend_address(
                db_id,
                &db_type,
                target_server_clone.as_ref(),
                provision_result.exposed_port,
            );

            // Generate SNI Traefik config
            traefik_service.generate_sni_config(
                &db_id.to_string(),
                &db_type,
                &fqdn_clone,
                &cert.cert_pem,
                &cert.key_pem,
                &backend_address,
            )?;

            // Update DB record
            DatabaseRepository::update_provisioned(
                &pool,
                db_id,
                &provision_result.container_id,
                &provision_result.network_id,
                Some(&cert.cert_pem),
            )
            .await?;

            // Record usage event: start
            let _ = BillingRepository::record_usage_event(&pool, db_id, "start", billing_user_id, &billing_db_name, billing_plan_id).await;

            // Notify SSE: provisioning → running
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id,
                database_id: db_id,
                event_type: "status_changed".to_string(),
                status: Some(DbStatus::Running),
            });

            Ok::<_, AppError>(())
        }
        .await;

        if let Err(e) = result {
            tracing::error!("Provisioning failed for {db_id}: {e}");
            let _ = DatabaseRepository::update_status(&pool, db_id, &DbStatus::Error).await;
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id,
                database_id: db_id,
                event_type: "status_changed".to_string(),
                status: Some(DbStatus::Error),
            });
            port_pool.release(port as u16);
        }
    });

    let standard_port = standard_port_for_db_type(&req.db_type);
    let ssl_mode_owned = ssl_mode.to_string();
    let connection_url = build_connection_url_sni(
        &req.db_type,
        &username,
        &password,
        &fqdn,
        db_name.as_deref(),
        &ssl_mode_owned,
    );

    Ok(Json(DatabaseResponse {
        id: db_instance.id,
        name: db_instance.name,
        db_type: db_instance.db_type,
        status: db_instance.status,
        host: fqdn,
        port: standard_port,
        username,
        password,
        database_name: db_name,
        connection_url,
        tls_enabled: true,
        ssl_mode: ssl_mode_owned,
        cpu_limit,
        memory_limit_mb,
        bundle_id: None,
        plan_template_id,
        subdomain: Some(subdomain),
        routing_mode: "sni".to_string(),
        created_at: db_instance.created_at,
    }))
}

pub async fn create_bundle(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateBundleRequest>,
) -> AppResult<Json<BundleResponse>> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Validate ssl_mode
    let ssl_mode = match req.ssl_mode.as_str() {
        "verify-ca" | "require" => req.ssl_mode.as_str(),
        _ => return Err(AppError::BadRequest("ssl_mode must be 'require' or 'verify-ca'".to_string())),
    };

    // Check limits (bundle creates 2 databases)
    let count = DatabaseRepository::count_by_user(&state.db, user.id).await?;
    let max = state.config.max_databases_per_user as i64;
    if count + 2 > max {
        return Err(AppError::BadRequest(format!(
            "Database limit reached ({max}). Bundle requires 2 slots."
        )));
    }

    // Resolve plan template if provided
    let (cpu_limit, memory_limit_mb, plan_template_id) = if let Some(template_id) = req.plan_template_id {
        let template = BillingRepository::get_plan_template(&state.db, template_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Plan template not found".to_string()))?;
        if !template.active {
            return Err(AppError::BadRequest("Plan template is not active".to_string()));
        }
        (template.cpu_limit, template.memory_limit_mb, Some(template_id))
    } else {
        (req.cpu_limit.unwrap_or(0.5), req.memory_limit_mb.unwrap_or(256), None)
    };
    let tls_mode = ssl_mode;

    // Select Docker server
    let target_server = if let Some(server_id) = req.server_id {
        let server = DockerServerRepository::find_active_client_by_id(&state.db, server_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Selected server not found or not available".to_string()))?;
        let count = DockerServerRepository::count_containers_on_server(&state.db, server.id).await?;
        if count + 2 > server.max_containers as i64 {
            return Err(AppError::BadRequest("Selected server doesn't have enough capacity for a bundle".to_string()));
        }
        Some(server)
    } else {
        DockerServerRepository::select_best_client_server(&state.db).await?
    };
    let docker_server_id = target_server.as_ref().map(|s| s.id);
    let target_server_clone = target_server.clone();

    // Create bundle record
    let bundle = DatabaseRepository::create_bundle(&state.db, user.id, &req.name).await?;

    // Generate credentials for PG
    let pg_username = format!("u_{}", generate_random_string(12));
    let pg_password = generate_random_string(32);
    let pg_db_name = req.name.clone();
    let pg_password_encrypted = encrypt_string(&pg_password, &state.config.encryption_key)?;
    let pg_port = state.port_pool.allocate()? as i32;

    // Generate credentials for Redis
    let redis_password = generate_random_string(32);
    let redis_password_encrypted = encrypt_string(&redis_password, &state.config.encryption_key)?;
    let redis_port = match state.port_pool.allocate() {
        Ok(p) => p as i32,
        Err(e) => {
            state.port_pool.release(pg_port as u16);
            return Err(e);
        }
    };

    let pg_name = format!("{}-pg", req.name);
    let redis_name = format!("{}-redis", req.name);

    // Generate subdomains
    let pg_subdomain = generate_subdomain(&pg_name, Uuid::new_v4());
    let redis_subdomain = generate_subdomain(&redis_name, Uuid::new_v4());

    // Create PG record
    let pg_instance = DatabaseRepository::create(
        &state.db,
        user.id,
        &pg_name,
        &DbType::Postgresql,
        &state.config.platform_domain,
        pg_port,
        &pg_username,
        &pg_password_encrypted,
        Some(&pg_db_name),
        cpu_limit,
        memory_limit_mb,
        Some(bundle.id),
        tls_mode,
        plan_template_id,
        docker_server_id,
        &pg_subdomain,
        "sni",
    )
    .await?;

    // Create Redis record
    let redis_instance = DatabaseRepository::create(
        &state.db,
        user.id,
        &redis_name,
        &DbType::Redis,
        &state.config.platform_domain,
        redis_port,
        "default",
        &redis_password_encrypted,
        None,
        cpu_limit,
        memory_limit_mb,
        Some(bundle.id),
        tls_mode,
        plan_template_id,
        docker_server_id,
        &redis_subdomain,
        "sni",
    )
    .await?;

    let bundle_id = bundle.id;
    let pg_id = pg_instance.id;
    let redis_id = redis_instance.id;
    let billing_user_id = user.id;
    let billing_pg_name = pg_name.clone();
    let billing_redis_name = redis_name.clone();
    let billing_plan_id = plan_template_id;

    let pg_fqdn = subdomain_fqdn(&pg_instance.subdomain, &state.config.platform_domain);
    let redis_fqdn = subdomain_fqdn(&redis_instance.subdomain, &state.config.platform_domain);

    // Provision async
    let provisioner = state.provisioner.clone();
    let pool = state.db.clone();
    let tls_service = state.tls_service.clone();
    let traefik_service = state.traefik_service.clone();
    let port_pool = state.port_pool.clone();
    let pg_fqdn_clone = pg_fqdn.clone();
    let redis_fqdn_clone = redis_fqdn.clone();

    let pg_username_clone = pg_username.clone();
    let pg_password_clone = pg_password.clone();
    let pg_db_name_clone = pg_db_name.clone();
    let redis_password_clone = redis_password.clone();
    let event_tx = state.event_tx.clone();

    tokio::spawn(async move {
        let result = async {
            let target_docker = if let Some(ref server) = target_server_clone {
                Some(ProvisionerService::connect_to_server(server)?)
            } else {
                None
            };

            // Determine if this is a remote server
            let is_remote = target_server_clone
                .as_ref()
                .map(|s| is_remote_server(&s.url))
                .unwrap_or(false);

            let pg_exposed_port = if is_remote { Some(pg_port as u16) } else { None };
            let redis_exposed_port = if is_remote { Some(redis_port as u16) } else { None };

            // Extract host IP for remote servers (bind on private IP instead of 0.0.0.0)
            let host_ip_owned = target_server_clone
                .as_ref()
                .and_then(|s| extract_server_ip(&s.url));

            // Generate TLS certs for subdomain FQDNs
            let pg_cert = tls_service.generate_cert_for_subdomain(&pg_fqdn_clone)?;
            let redis_cert = tls_service.generate_cert_for_subdomain(&redis_fqdn_clone)?;

            // Provision PG in SNI mode
            let pg_result = provisioner
                .create_database_sni(
                    target_docker.as_ref(),
                    pg_id,
                    &DbType::Postgresql,
                    &pg_username_clone,
                    &pg_password_clone,
                    &pg_db_name_clone,
                    cpu_limit,
                    memory_limit_mb as i64,
                    is_remote,
                    pg_exposed_port,
                    host_ip_owned.as_deref(),
                )
                .await?;

            // Provision Redis in SNI mode
            let redis_result = provisioner
                .create_database_sni(
                    target_docker.as_ref(),
                    redis_id,
                    &DbType::Redis,
                    "default",
                    &redis_password_clone,
                    "default",
                    cpu_limit,
                    memory_limit_mb as i64,
                    is_remote,
                    redis_exposed_port,
                    host_ip_owned.as_deref(),
                )
                .await?;

            // Wait for both containers to become healthy
            let fallback = provisioner.fallback_docker();
            let docker_ref = target_docker.as_ref().unwrap_or(fallback);
            ProvisionerService::wait_for_healthy(docker_ref, &pg_result.container_id, 90).await?;
            ProvisionerService::wait_for_healthy(docker_ref, &redis_result.container_id, 90).await?;

            // Bundle network — use PG's network
            DatabaseRepository::update_bundle_network(&pool, bundle_id, &pg_result.network_id)
                .await?;

            // Compute backend addresses
            let pg_backend = compute_backend_address(
                pg_id, &DbType::Postgresql, target_server_clone.as_ref(), pg_result.exposed_port,
            );
            let redis_backend = compute_backend_address(
                redis_id, &DbType::Redis, target_server_clone.as_ref(), redis_result.exposed_port,
            );

            // SNI Traefik configs
            traefik_service.generate_sni_config(
                &pg_id.to_string(),
                &DbType::Postgresql,
                &pg_fqdn_clone,
                &pg_cert.cert_pem,
                &pg_cert.key_pem,
                &pg_backend,
            )?;
            traefik_service.generate_sni_config(
                &redis_id.to_string(),
                &DbType::Redis,
                &redis_fqdn_clone,
                &redis_cert.cert_pem,
                &redis_cert.key_pem,
                &redis_backend,
            )?;

            // Update DB records
            DatabaseRepository::update_provisioned(
                &pool,
                pg_id,
                &pg_result.container_id,
                &pg_result.network_id,
                Some(&pg_cert.cert_pem),
            )
            .await?;
            DatabaseRepository::update_provisioned(
                &pool,
                redis_id,
                &redis_result.container_id,
                &redis_result.network_id,
                Some(&redis_cert.cert_pem),
            )
            .await?;

            // Record usage events
            let _ = BillingRepository::record_usage_event(&pool, pg_id, "start", billing_user_id, &billing_pg_name, billing_plan_id).await;
            let _ = BillingRepository::record_usage_event(&pool, redis_id, "start", billing_user_id, &billing_redis_name, billing_plan_id).await;

            // Notify SSE: bundle provisioned
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id, database_id: pg_id,
                event_type: "status_changed".to_string(), status: Some(DbStatus::Running),
            });
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id, database_id: redis_id,
                event_type: "status_changed".to_string(), status: Some(DbStatus::Running),
            });

            Ok::<_, AppError>(())
        }
        .await;

        if let Err(e) = result {
            tracing::error!("Bundle provisioning failed for {bundle_id}: {e}");
            let _ = DatabaseRepository::update_status(&pool, pg_id, &DbStatus::Error).await;
            let _ = DatabaseRepository::update_status(&pool, redis_id, &DbStatus::Error).await;
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id, database_id: pg_id,
                event_type: "status_changed".to_string(), status: Some(DbStatus::Error),
            });
            let _ = event_tx.send(DbEvent {
                user_id: billing_user_id, database_id: redis_id,
                event_type: "status_changed".to_string(), status: Some(DbStatus::Error),
            });
            port_pool.release(pg_port as u16);
            port_pool.release(redis_port as u16);
        }
    });

    let ssl_mode_owned = ssl_mode.to_string();
    let pg_connection_url = build_connection_url_sni(
        &DbType::Postgresql,
        &pg_username,
        &pg_password,
        &pg_fqdn,
        Some(&pg_db_name),
        &ssl_mode_owned,
    );
    let redis_connection_url = build_connection_url_sni(
        &DbType::Redis,
        "default",
        &redis_password,
        &redis_fqdn,
        None,
        &ssl_mode_owned,
    );

    Ok(Json(BundleResponse {
        id: bundle.id,
        name: req.name.clone(),
        postgresql: DatabaseResponse {
            id: pg_instance.id,
            name: pg_name,
            db_type: DbType::Postgresql,
            status: DbStatus::Provisioning,
            host: pg_fqdn,
            port: 5432,
            username: pg_username,
            password: pg_password,
            database_name: Some(pg_db_name),
            connection_url: pg_connection_url,
            tls_enabled: true,
            ssl_mode: ssl_mode_owned.clone(),
            cpu_limit,
            memory_limit_mb,
            bundle_id: Some(bundle.id),
            plan_template_id,
            subdomain: Some(pg_instance.subdomain),
            routing_mode: "sni".to_string(),
            created_at: pg_instance.created_at,
        },
        redis: DatabaseResponse {
            id: redis_instance.id,
            name: redis_name,
            db_type: DbType::Redis,
            status: DbStatus::Provisioning,
            host: redis_fqdn,
            port: 6379,
            username: "default".to_string(),
            password: redis_password,
            database_name: None,
            connection_url: redis_connection_url,
            tls_enabled: true,
            ssl_mode: ssl_mode_owned,
            cpu_limit,
            memory_limit_mb,
            bundle_id: Some(bundle.id),
            plan_template_id,
            subdomain: Some(redis_instance.subdomain),
            routing_mode: "sni".to_string(),
            created_at: redis_instance.created_at,
        },
        created_at: bundle.created_at,
    }))
}

pub async fn container_action(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<ContainerActionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned yet".to_string()))?;

    // Resolve Docker client for this instance's server
    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    let docker_for_health = target_docker.as_ref().unwrap_or(state.provisioner.fallback_docker());

    let new_status = match req.action {
        ContainerAction::Start => {
            state.provisioner.start_container(target_docker.as_ref(), container_id).await?;
            ProvisionerService::wait_for_healthy(docker_for_health, container_id, 60).await?;
            DatabaseRepository::update_status(&state.db, id, &DbStatus::Running).await?;
            let _ = BillingRepository::record_usage_event(&state.db, id, "start", inst.user_id, &inst.name, inst.plan_template_id).await;
            DbStatus::Running
        }
        ContainerAction::Stop => {
            state.provisioner.stop_container(target_docker.as_ref(), container_id).await?;
            DatabaseRepository::update_status(&state.db, id, &DbStatus::Stopped).await?;
            let _ = BillingRepository::record_usage_event(&state.db, id, "stop", inst.user_id, &inst.name, inst.plan_template_id).await;
            DbStatus::Stopped
        }
        ContainerAction::Restart => {
            state.provisioner.restart_container(target_docker.as_ref(), container_id).await?;
            ProvisionerService::wait_for_healthy(docker_for_health, container_id, 60).await?;
            DatabaseRepository::update_status(&state.db, id, &DbStatus::Running).await?;
            DbStatus::Running
        }
    };

    let _ = state.event_tx.send(DbEvent {
        user_id: inst.user_id,
        database_id: id,
        event_type: "status_changed".to_string(),
        status: Some(new_status),
    });

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn list_database_users(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<DatabaseUserListItem>>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let users = DatabaseRepository::find_database_users(&state.db, id).await?;
    let items: Vec<DatabaseUserListItem> = users
        .into_iter()
        .map(|u| DatabaseUserListItem {
            id: u.id,
            database_id: u.database_id,
            username: u.username,
            permission: u.permission,
            created_at: u.created_at,
        })
        .collect();

    Ok(Json(items))
}

pub async fn create_database_user(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateDatabaseUserRequest>,
) -> AppResult<Json<DatabaseUserResponse>> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if inst.status != DbStatus::Running {
        return Err(AppError::BadRequest(
            "Database must be running to create users".to_string(),
        ));
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned".to_string()))?;

    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    // Generate password
    let password = generate_random_string(32);
    let password_encrypted = encrypt_string(&password, &state.config.encryption_key)?;

    // Decrypt owner password for exec commands
    let owner_password =
        decrypt_string(&inst.password_encrypted, &state.config.encryption_key)?;

    // Create user in the actual database
    match inst.db_type {
        DbType::Postgresql => {
            let db_name = inst
                .database_name
                .as_deref()
                .ok_or_else(|| AppError::Internal("PG instance missing database_name".to_string()))?;
            state
                .provisioner
                .create_pg_user(
                    target_docker.as_ref(),
                    container_id,
                    &inst.username,
                    db_name,
                    &req.username,
                    &password,
                    &req.permission,
                )
                .await?;
        }
        DbType::Redis => {
            state
                .provisioner
                .create_redis_user(
                    target_docker.as_ref(),
                    container_id,
                    &owner_password,
                    &req.username,
                    &password,
                    &req.permission,
                )
                .await?;
        }
        DbType::Mariadb => {
            let db_name = inst
                .database_name
                .as_deref()
                .ok_or_else(|| AppError::Internal("MariaDB instance missing database_name".to_string()))?;
            state
                .provisioner
                .create_mariadb_user(
                    target_docker.as_ref(),
                    container_id,
                    &owner_password,
                    db_name,
                    &req.username,
                    &password,
                    &req.permission,
                )
                .await?;
        }
    }

    // Save to platform DB
    let db_user = DatabaseRepository::create_database_user(
        &state.db,
        id,
        &req.username,
        &password_encrypted,
        &req.permission,
    )
    .await?;

    Ok(Json(DatabaseUserResponse {
        id: db_user.id,
        database_id: db_user.database_id,
        username: db_user.username,
        password,
        permission: db_user.permission,
        created_at: db_user.created_at,
    }))
}

pub async fn delete_database_user(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((db_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let db_user = DatabaseRepository::find_database_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database user not found".to_string()))?;

    if db_user.database_id != db_id {
        return Err(AppError::NotFound("Database user not found".to_string()));
    }

    if inst.status == DbStatus::Running {
        let container_id = inst.container_id.as_deref().unwrap_or_default();
        let owner_password =
            decrypt_string(&inst.password_encrypted, &state.config.encryption_key)?;
        let target_docker = resolve_docker_for_instance(&state, &inst).await?;

        match inst.db_type {
            DbType::Postgresql => {
                let db_name = inst.database_name.as_deref().unwrap_or("postgres");
                let _ = state
                    .provisioner
                    .remove_pg_user(target_docker.as_ref(), container_id, &inst.username, db_name, &db_user.username)
                    .await;
            }
            DbType::Redis => {
                let _ = state
                    .provisioner
                    .remove_redis_user(target_docker.as_ref(), container_id, &owner_password, &db_user.username)
                    .await;
            }
            DbType::Mariadb => {
                let _ = state
                    .provisioner
                    .remove_mariadb_user(target_docker.as_ref(), container_id, &owner_password, &db_user.username)
                    .await;
            }
        }
    }

    DatabaseRepository::delete_database_user(&state.db, user_id).await?;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

pub async fn rotate_user_password(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((db_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if inst.status != DbStatus::Running {
        return Err(AppError::BadRequest(
            "Database must be running to rotate passwords".to_string(),
        ));
    }

    let db_user = DatabaseRepository::find_database_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database user not found".to_string()))?;

    if db_user.database_id != db_id {
        return Err(AppError::NotFound("Database user not found".to_string()));
    }

    let container_id = inst.container_id.as_deref().unwrap_or_default();
    let owner_password =
        decrypt_string(&inst.password_encrypted, &state.config.encryption_key)?;
    let new_password = generate_random_string(32);
    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    match inst.db_type {
        DbType::Postgresql => {
            let db_name = inst.database_name.as_deref().unwrap_or("postgres");
            state
                .provisioner
                .rotate_pg_password(
                    target_docker.as_ref(),
                    container_id,
                    &inst.username,
                    db_name,
                    &db_user.username,
                    &new_password,
                )
                .await?;
        }
        DbType::Redis => {
            state
                .provisioner
                .rotate_redis_password(
                    target_docker.as_ref(),
                    container_id,
                    &owner_password,
                    &db_user.username,
                    &new_password,
                )
                .await?;
        }
        DbType::Mariadb => {
            state
                .provisioner
                .rotate_mariadb_password(
                    target_docker.as_ref(),
                    container_id,
                    &owner_password,
                    &db_user.username,
                    &new_password,
                )
                .await?;
        }
    }

    let new_encrypted = encrypt_string(&new_password, &state.config.encryption_key)?;
    DatabaseRepository::update_database_user_password(&state.db, user_id, &new_encrypted).await?;

    Ok(Json(serde_json::json!({ "password": new_password })))
}

pub async fn rotate_owner_password(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if inst.status != DbStatus::Running {
        return Err(AppError::BadRequest(
            "Database must be running to rotate passwords".to_string(),
        ));
    }

    let container_id = inst.container_id.as_deref().unwrap_or_default();
    let old_password =
        decrypt_string(&inst.password_encrypted, &state.config.encryption_key)?;
    let new_password = generate_random_string(32);
    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    match inst.db_type {
        DbType::Postgresql => {
            let db_name = inst.database_name.as_deref().unwrap_or("postgres");
            state
                .provisioner
                .rotate_pg_password(
                    target_docker.as_ref(),
                    container_id,
                    &inst.username,
                    db_name,
                    &inst.username,
                    &new_password,
                )
                .await?;
        }
        DbType::Redis => {
            // For Redis owner (default user), update requirepass
            let cmd = format!("CONFIG SET requirepass {new_password}");
            state
                .provisioner
                .exec_in_container(
                    target_docker.as_ref(),
                    container_id,
                    vec!["redis-cli", "-a", &old_password, "--no-auth-warning", &cmd],
                )
                .await?;
            // Persist config
            state
                .provisioner
                .exec_in_container(
                    target_docker.as_ref(),
                    container_id,
                    vec![
                        "redis-cli",
                        "-a",
                        &new_password,
                        "--no-auth-warning",
                        "CONFIG",
                        "REWRITE",
                    ],
                )
                .await?;
        }
        DbType::Mariadb => {
            state
                .provisioner
                .rotate_mariadb_password(
                    target_docker.as_ref(),
                    container_id,
                    &old_password,
                    &inst.username,
                    &new_password,
                )
                .await?;
        }
    }

    let new_encrypted = encrypt_string(&new_password, &state.config.encryption_key)?;
    DatabaseRepository::update_owner_password(&state.db, id, &new_encrypted).await?;

    Ok(Json(
        serde_json::json!({ "password": new_password }),
    ))
}

pub async fn list_databases(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<DatabaseResponse>>> {
    let instances = DatabaseRepository::find_by_user(&state.db, user.id).await?;

    let mut responses = Vec::new();
    for inst in instances {
        let password = decrypt_string(&inst.password_encrypted, &state.config.encryption_key)
            .unwrap_or_else(|_| "***".to_string());
        let ssl_mode = inst.tls_mode.clone();
        let tls_enabled = ssl_mode != "disabled";

        let (host, port, connection_url) = if inst.routing_mode == "sni" {
            let fqdn = subdomain_fqdn(&inst.subdomain, &state.config.platform_domain);
            let std_port = standard_port_for_db_type(&inst.db_type);
            let url = build_connection_url_sni(
                &inst.db_type, &inst.username, &password, &fqdn, inst.database_name.as_deref(), &ssl_mode,
            );
            (fqdn, std_port, url)
        } else {
            let url = build_connection_url(
                &inst.db_type, &inst.username, &password, &inst.host, inst.port,
                inst.database_name.as_deref(), &inst.tls_mode,
            );
            (inst.host.clone(), inst.port, url)
        };

        responses.push(DatabaseResponse {
            id: inst.id,
            name: inst.name,
            db_type: inst.db_type,
            status: inst.status,
            host,
            port,
            username: inst.username,
            password,
            database_name: inst.database_name,
            connection_url,
            tls_enabled,
            ssl_mode,
            cpu_limit: inst.cpu_limit,
            memory_limit_mb: inst.memory_limit_mb,
            bundle_id: inst.bundle_id,
            plan_template_id: inst.plan_template_id,
            subdomain: Some(inst.subdomain),
            routing_mode: inst.routing_mode,
            created_at: inst.created_at,
        });
    }

    Ok(Json(responses))
}

pub async fn get_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DatabaseResponse>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let password = decrypt_string(&inst.password_encrypted, &state.config.encryption_key)
        .unwrap_or_else(|_| "***".to_string());
    let ssl_mode = inst.tls_mode.clone();
    let tls_enabled = ssl_mode != "disabled";

    let (host, port, connection_url) = if inst.routing_mode == "sni" {
        let fqdn = subdomain_fqdn(&inst.subdomain, &state.config.platform_domain);
        let std_port = standard_port_for_db_type(&inst.db_type);
        let url = build_connection_url_sni(
            &inst.db_type, &inst.username, &password, &fqdn, inst.database_name.as_deref(), &ssl_mode,
        );
        (fqdn, std_port, url)
    } else {
        let url = build_connection_url(
            &inst.db_type, &inst.username, &password, &inst.host, inst.port,
            inst.database_name.as_deref(), &inst.tls_mode,
        );
        (inst.host.clone(), inst.port, url)
    };

    Ok(Json(DatabaseResponse {
        id: inst.id,
        name: inst.name,
        db_type: inst.db_type,
        status: inst.status,
        host,
        port,
        username: inst.username,
        password,
        database_name: inst.database_name,
        connection_url,
        tls_enabled,
        ssl_mode,
        cpu_limit: inst.cpu_limit,
        memory_limit_mb: inst.memory_limit_mb,
        bundle_id: inst.bundle_id,
        plan_template_id: inst.plan_template_id,
        subdomain: Some(inst.subdomain),
        routing_mode: inst.routing_mode,
        created_at: inst.created_at,
    }))
}

pub async fn delete_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Record stop event before deletion
    let _ = BillingRepository::record_usage_event(&state.db, id, "stop", inst.user_id, &inst.name, inst.plan_template_id).await;

    // Mark as deleting
    DatabaseRepository::update_status(&state.db, id, &DbStatus::Deleting).await?;

    // Resolve Docker client for this instance
    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    // Detach from any private networks before deletion
    if let Some(ref container_id) = inst.container_id {
        if let Ok(networks) = PrivateNetworkRepository::find_networks_for_database(&state.db, id).await {
            for net in &networks {
                if let Some(ref docker_net_id) = net.docker_network_id {
                    let _ = state.provisioner.detach_container_from_network(
                        target_docker.as_ref(), container_id, docker_net_id,
                    ).await;
                }
            }
        }
    }

    // Notify SSE: deleting
    let _ = state.event_tx.send(DbEvent {
        user_id: inst.user_id,
        database_id: id,
        event_type: "status_changed".to_string(),
        status: Some(DbStatus::Deleting),
    });

    // Async cleanup
    let provisioner = state.provisioner.clone();
    let traefik_service = state.traefik_service.clone();
    let pool = state.db.clone();
    let port_pool = state.port_pool.clone();
    let port = inst.port as u16;
    let event_tx = state.event_tx.clone();
    let delete_user_id = inst.user_id;

    tokio::spawn(async move {
        if let (Some(container_id), Some(network_id)) = (&inst.container_id, &inst.network_id) {
            if let Err(e) = provisioner.remove_container(target_docker.as_ref(), container_id, network_id).await {
                tracing::error!("Failed to remove container: {e}");
            }
        }

        let _ = traefik_service.remove_config(&id.to_string());
        let _ = DatabaseRepository::delete(&pool, id).await;
        port_pool.release(port);

        // Notify SSE: deleted
        let _ = event_tx.send(DbEvent {
            user_id: delete_user_id,
            database_id: id,
            event_type: "deleted".to_string(),
            status: None,
        });
    });

    Ok(Json(serde_json::json!({ "status": "deleting" })))
}

pub async fn get_ca_cert(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<String> {
    state.tls_service.get_ca_cert()
}

// --- Backup endpoints ---

pub async fn create_backup(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BackupRecord>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if inst.status != DbStatus::Running {
        return Err(AppError::BadRequest(
            "Database must be running to create backups".to_string(),
        ));
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned".to_string()))?;

    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let extension = match inst.db_type {
        DbType::Postgresql => "dump",
        DbType::Redis => "rdb",
        DbType::Mariadb => "sql",
    };
    let filename = format!("{}_{}_{}.{}", inst.name, inst.db_type_str(), timestamp, extension);
    let backup_dir = &state.config.backup_dir;

    let owner_password =
        decrypt_string(&inst.password_encrypted, &state.config.encryption_key)?;

    let size = match inst.db_type {
        DbType::Postgresql => {
            let db_name = inst.database_name.as_deref().unwrap_or("postgres");
            state
                .provisioner
                .backup_postgres(target_docker.as_ref(), container_id, &inst.username, db_name, backup_dir, &filename)
                .await?
        }
        DbType::Redis => {
            state
                .provisioner
                .backup_redis(target_docker.as_ref(), container_id, &owner_password, backup_dir, &filename)
                .await?
        }
        DbType::Mariadb => {
            let db_name = inst.database_name.as_deref().unwrap_or("default");
            state
                .provisioner
                .backup_mariadb(target_docker.as_ref(), container_id, &inst.username, &owner_password, db_name, backup_dir, &filename)
                .await?
        }
    };

    let backup = DatabaseRepository::create_backup(&state.db, id, &filename, size as i64).await?;

    Ok(Json(backup))
}

pub async fn list_backups(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<BackupRecord>>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let backups = DatabaseRepository::find_backups_by_database(&state.db, id).await?;
    Ok(Json(backups))
}

pub async fn delete_backup(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((db_id, backup_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let backup = DatabaseRepository::find_backup_by_id(&state.db, backup_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Backup not found".to_string()))?;

    if backup.database_id != db_id {
        return Err(AppError::NotFound("Backup not found".to_string()));
    }

    // Delete file (with path traversal protection)
    let backup_dir = std::path::Path::new(&state.config.backup_dir).canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&state.config.backup_dir));
    let path = backup_dir.join(&backup.filename);
    if path.starts_with(&backup_dir) {
        let _ = tokio::fs::remove_file(&path).await;
    } else {
        tracing::warn!("Path traversal attempt blocked for backup: {}", backup.filename);
    }

    DatabaseRepository::delete_backup(&state.db, backup_id).await?;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

pub async fn database_stats(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned yet".to_string()))?;

    if inst.status != DbStatus::Running {
        return Ok(Json(serde_json::json!({
            "status": inst.status,
            "cpu_percent": 0,
            "memory_usage_bytes": 0,
            "memory_limit_bytes": 0,
        })));
    }

    let target_docker = resolve_docker_for_instance(&state, &inst).await?;
    let fallback_docker;
    let docker = if let Some(ref d) = target_docker {
        d
    } else {
        fallback_docker = crate::utils::docker::create_docker_client(&state.config.docker_host)
            .map_err(|e| AppError::Internal(format!("Docker connection failed: {e}")))?;
        &fallback_docker
    };

    // Get one-shot stats
    use bollard::query_parameters::StatsOptions;
    use futures_util::StreamExt;

    let options = StatsOptions { stream: false, one_shot: true };
    let mut stream = docker.stats(container_id, Some(options));

    let (cpu_pct, mem_usage, mem_limit) = if let Some(Ok(stats)) = stream.next().await {
        let cpu_stats = stats.cpu_stats.unwrap_or_default();
        let precpu_stats = stats.precpu_stats.unwrap_or_default();
        let cpu_delta = cpu_stats.cpu_usage.unwrap_or_default().total_usage.unwrap_or(0) as f64
            - precpu_stats.cpu_usage.unwrap_or_default().total_usage.unwrap_or(0) as f64;
        let system_delta = cpu_stats.system_cpu_usage.unwrap_or(0) as f64
            - precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
        let num_cpus = cpu_stats.online_cpus.unwrap_or(1) as f64;
        let cpu_pct = if system_delta > 0.0 {
            (cpu_delta / system_delta) * num_cpus * 100.0
        } else {
            0.0
        };
        let mem_stats = stats.memory_stats.unwrap_or_default();
        (cpu_pct, mem_stats.usage.unwrap_or(0) as i64, mem_stats.limit.unwrap_or(0) as i64)
    } else {
        (0.0, 0, 0)
    };

    // Get disk usage
    let disk_usage = state
        .provisioner
        .get_disk_usage(target_docker.as_ref(), container_id, &inst.db_type)
        .await
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "status": inst.status,
        "cpu_percent": cpu_pct,
        "memory_usage_bytes": mem_usage,
        "memory_limit_bytes": mem_limit,
        "cpu_limit": inst.cpu_limit,
        "memory_limit_mb": inst.memory_limit_mb,
        "disk_usage_bytes": disk_usage,
    })))
}

pub async fn list_available_servers(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let servers = DockerServerRepository::list_active_client(&state.db).await?;
    let result: Vec<serde_json::Value> = servers
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "region": s.region,
            })
        })
        .collect();
    Ok(Json(result))
}

fn build_connection_url(
    db_type: &DbType,
    username: &str,
    password: &str,
    host: &str,
    port: i32,
    db_name: Option<&str>,
    tls_mode: &str,
) -> String {
    let tls_enabled = tls_mode == "enabled";
    match db_type {
        DbType::Postgresql => {
            let ssl_param = if tls_enabled { "sslmode=verify-ca" } else { "sslmode=disable" };
            format!(
                "postgresql://{}:{}@{}:{}/{}?{}",
                username, password, host, port,
                db_name.unwrap_or("postgres"),
                ssl_param,
            )
        }
        DbType::Redis => {
            let scheme = if tls_enabled { "rediss" } else { "redis" };
            format!("{}://:{}@{}:{}", scheme, password, host, port)
        }
        DbType::Mariadb => {
            let ssl_param = if tls_enabled { "?ssl-mode=VERIFY_CA" } else { "" };
            format!(
                "mysql://{}:{}@{}:{}/{}{}",
                username, password, host, port,
                db_name.unwrap_or("default"),
                ssl_param,
            )
        }
    }
}

/// Build connection URL for SNI-routed databases (TLS always, standard ports).
/// ssl_mode: "verify-ca" (full TLS with CA cert) or "require" (encrypted, no cert needed)
fn build_connection_url_sni(
    db_type: &DbType,
    username: &str,
    password: &str,
    fqdn: &str,
    db_name: Option<&str>,
    ssl_mode: &str,
) -> String {
    match db_type {
        DbType::Postgresql => {
            let pg_ssl = if ssl_mode == "verify-ca" { "sslmode=verify-ca" } else { "sslmode=require" };
            format!(
                "postgresql://{}:{}@{}:{}/{}?{}",
                username, password, fqdn, 5432,
                db_name.unwrap_or("postgres"),
                pg_ssl,
            )
        }
        DbType::Redis => {
            // Redis always uses rediss:// (TLS) — no cert verification distinction in URL
            format!("rediss://:{}@{}:{}", password, fqdn, 6379)
        }
        DbType::Mariadb => {
            let maria_ssl = if ssl_mode == "verify-ca" { "ssl-mode=VERIFY_CA" } else { "ssl-mode=REQUIRED" };
            format!(
                "mysql://{}:{}@{}:{}/{}?{}",
                username, password, fqdn, 3306,
                db_name.unwrap_or("default"),
                maria_ssl,
            )
        }
    }
}

/// Extract IP address from a Docker server URL (e.g. "tcp://192.168.1.12:2376" -> "192.168.1.12").
fn extract_server_ip(url: &str) -> Option<String> {
    // Handle tcp://host:port, https://host:port, etc.
    let without_scheme = url
        .find("://")
        .map(|i| &url[i + 3..])
        .unwrap_or(url);
    // Strip port if present
    let host = if without_scheme.contains(':') {
        without_scheme.split(':').next().unwrap_or(without_scheme)
    } else {
        without_scheme
    };
    if host.is_empty() || host == "localhost" || host.starts_with('/') {
        None
    } else {
        Some(host.to_string())
    }
}

/// Check if a Docker server URL points to a remote host (not local/unix socket).
fn is_remote_server(url: &str) -> bool {
    url != "local"
        && !url.starts_with("unix://")
        && !url.contains("localhost")
        && !url.contains("127.0.0.1")
}

/// Compute the backend address for Traefik SNI routing.
/// Local: container DNS name on shared proxy network.
/// Remote: server IP + exposed port.
fn compute_backend_address(
    db_id: uuid::Uuid,
    db_type: &DbType,
    server: Option<&crate::models::DockerServer>,
    exposed_port: Option<u16>,
) -> String {
    let internal_port = standard_port_for_db_type(db_type);
    if let Some(srv) = server {
        if is_remote_server(&srv.url) {
            if let (Some(ip), Some(port)) = (extract_server_ip(&srv.url), exposed_port) {
                return format!("{ip}:{port}");
            }
        }
    }
    // Local: use Docker DNS
    format!("sb-{db_id}:{internal_port}")
}

/// Standard port for a given database type.
fn standard_port_for_db_type(db_type: &DbType) -> i32 {
    match db_type {
        DbType::Postgresql => 5432,
        DbType::Redis => 6379,
        DbType::Mariadb => 3306,
    }
}

// ── Backup Schedule ──────────────────────────────────────────────────

pub async fn create_backup_schedule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<crate::models::backup_schedule::CreateBackupScheduleRequest>,
) -> AppResult<Json<crate::models::backup_schedule::BackupSchedule>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let schedule = crate::repository::BackupScheduleRepository::create(
        &state.db,
        id,
        req.interval_hours.unwrap_or(24),
        req.retention_count.unwrap_or(7),
        req.enabled.unwrap_or(true),
    )
    .await?;
    Ok(Json(schedule))
}

pub async fn get_backup_schedule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let schedule = crate::repository::BackupScheduleRepository::find_by_database(&state.db, id).await?;
    match schedule {
        Some(s) => Ok(Json(serde_json::to_value(s).unwrap())),
        None => Ok(Json(serde_json::json!(null))),
    }
}

pub async fn update_backup_schedule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<crate::models::backup_schedule::UpdateBackupScheduleRequest>,
) -> AppResult<Json<crate::models::backup_schedule::BackupSchedule>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let schedule = crate::repository::BackupScheduleRepository::update(
        &state.db,
        id,
        req.interval_hours,
        req.retention_count,
        req.enabled,
    )
    .await?;
    Ok(Json(schedule))
}

pub async fn delete_backup_schedule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }
    crate::repository::BackupScheduleRepository::delete(&state.db, id).await?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// ── Export / Download ────────────────────────────────────────────────

pub async fn export_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned".to_string()))?;

    let target_docker = resolve_docker_for_instance(&state, &inst).await?;
    let password = decrypt_string(&inst.password_encrypted, &state.config.encryption_key)
        .map_err(|_| AppError::Internal("Decryption failed".to_string()))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.sql", inst.name, timestamp);
    let backup_dir = &state.config.backup_dir;

    let size_bytes = match inst.db_type {
        DbType::Postgresql => {
            state.provisioner.backup_postgres(
                target_docker.as_ref(), container_id, &inst.username,
                inst.database_name.as_deref().unwrap_or(&inst.name), backup_dir, &filename,
            ).await?
        }
        DbType::Mariadb => {
            state.provisioner.backup_mariadb(
                target_docker.as_ref(), container_id, &inst.username, &password,
                inst.database_name.as_deref().unwrap_or(&inst.name), backup_dir, &filename,
            ).await?
        }
        DbType::Redis => {
            state.provisioner.backup_redis(target_docker.as_ref(), container_id, &password, backup_dir, &filename).await?
        }
    };

    Ok(Json(serde_json::json!({
        "filename": filename,
        "size_bytes": size_bytes,
    })))
}

pub async fn download_export(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((id, filename)): Path<(Uuid, String)>,
) -> AppResult<axum::response::Response> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Sanitize filename — only allow alphanumeric, dots, underscores, hyphens
    if filename.contains("..") || filename.contains('/') || filename.contains('\\')
        || !filename.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(AppError::BadRequest("Invalid filename".to_string()));
    }

    let file_path = format!("{}/{}", state.config.backup_dir, filename);
    let data = tokio::fs::read(&file_path)
        .await
        .map_err(|_| AppError::NotFound("Export file not found".to_string()))?;

    use axum::response::IntoResponse;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/octet-stream"),
            (axum::http::header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
        ],
        data,
    ).into_response())
}

// ── Vertical Scaling ─────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ScaleRequest {
    pub plan_template_id: Uuid,
}

pub async fn scale_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<ScaleRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let container_id = inst
        .container_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Container not provisioned".to_string()))?;

    // Validate new plan
    let new_plan = BillingRepository::get_plan_template(&state.db, req.plan_template_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan not found".to_string()))?;

    // Check same db_type
    if new_plan.db_type != inst.db_type {
        return Err(AppError::BadRequest("Plan db_type does not match database type".to_string()));
    }
    if !new_plan.active {
        return Err(AppError::BadRequest("Plan is not active".to_string()));
    }

    let target_docker = resolve_docker_for_instance(&state, &inst).await?;

    // Update container resources (no restart needed)
    state.provisioner.update_container_resources(
        target_docker.as_ref(),
        container_id,
        new_plan.cpu_limit,
        new_plan.memory_limit_mb,
    ).await?;

    // Stop billing for old plan, start billing for new plan
    let _ = BillingRepository::record_usage_event(
        &state.db, id, "stop", user.id, &inst.name, inst.plan_template_id,
    ).await;
    let _ = BillingRepository::record_usage_event(
        &state.db, id, "start", user.id, &inst.name, Some(req.plan_template_id),
    ).await;

    // Update database record
    sqlx::query(
        "UPDATE database_instances SET plan_template_id = $1, cpu_limit = $2, memory_limit_mb = $3, updated_at = NOW() WHERE id = $4"
    )
    .bind(req.plan_template_id)
    .bind(new_plan.cpu_limit)
    .bind(new_plan.memory_limit_mb)
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "status": "scaled",
        "plan_template_id": req.plan_template_id,
        "cpu_limit": new_plan.cpu_limit,
        "memory_limit_mb": new_plan.memory_limit_mb,
    })))
}

// ── Disk Usage ───────────────────────────────────────────────────────
// (integrated into database_stats — see modification below)

// ── Favorites ────────────────────────────────────────────────────────

pub async fn add_favorite(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }
    crate::repository::FavoriteRepository::add(&state.db, user.id, id).await?;
    Ok(Json(serde_json::json!({ "status": "added" })))
}

pub async fn remove_favorite(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    crate::repository::FavoriteRepository::remove(&state.db, user.id, id).await?;
    Ok(Json(serde_json::json!({ "status": "removed" })))
}

pub async fn list_favorites(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<Uuid>>> {
    let ids = crate::repository::FavoriteRepository::list_by_user(&state.db, user.id).await?;
    Ok(Json(ids))
}

// ── Rename ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct RenameRequest {
    pub name: String,
}

pub async fn rename_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<RenameRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let inst = DatabaseRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;
    if inst.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if req.name.is_empty() || req.name.len() > 63 {
        return Err(AppError::BadRequest("Name must be 1-63 characters".to_string()));
    }

    sqlx::query("UPDATE database_instances SET name = $1, updated_at = NOW() WHERE id = $2")
        .bind(&req.name)
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "status": "renamed", "name": req.name })))
}

// ── Clone from Backup ────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CloneRequest {
    pub name: String,
}

pub async fn clone_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((db_id, backup_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CloneRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = claims.sub;

    let source = DatabaseRepository::find_by_id(&state.db, db_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Source database not found".to_string()))?;
    if source.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    let backup = DatabaseRepository::find_backup_by_id(&state.db, backup_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Backup not found".to_string()))?;
    if backup.database_id != db_id {
        return Err(AppError::BadRequest("Backup does not belong to this database".to_string()));
    }

    // Check quota
    let count = DatabaseRepository::count_by_user(&state.db, user_id).await?;
    if count >= state.config.max_databases_per_user as i64 {
        return Err(AppError::BadRequest("Maximum databases limit reached".to_string()));
    }

    // Allocate port
    let port = state.port_pool.allocate()?;

    let password = generate_random_string(24);
    let enc_password = encrypt_string(&password, &state.config.encryption_key)
        .map_err(|_| AppError::Internal("Encryption failed".to_string()))?;

    let subdomain = generate_subdomain(&req.name, uuid::Uuid::new_v4());

    // Create the new database record
    let new_db = DatabaseRepository::create(
        &state.db,
        user_id,
        &req.name,
        &source.db_type,
        &state.config.platform_ip,
        port as i32,
        &source.username,
        &enc_password,
        source.database_name.as_deref(),
        source.cpu_limit,
        source.memory_limit_mb,
        None,
        &source.tls_mode,
        source.plan_template_id,
        source.docker_server_id,
        &subdomain,
        "sni",
    )
    .await?;

    let new_db_id = new_db.id;

    // Spawn async provisioning + restore
    let pool = state.db.clone();
    let provisioner = state.provisioner.clone();
    let tls_service = state.tls_service.clone();
    let traefik_service = state.traefik_service.clone();
    let config = state.config.clone();
    let event_tx = state.event_tx.clone();
    let backup_path = format!("{}/{}", config.backup_dir, backup.filename);
    let source_db_type = source.db_type.clone();
    let username = source.username.clone();
    let clone_name = req.name.clone();

    tokio::spawn(async move {
        // Determine target server
        let target_server = if let Some(server_id) = source.docker_server_id {
            DockerServerRepository::find_by_id(&pool, server_id).await.ok().flatten()
        } else {
            None
        };
        let target_docker = target_server.as_ref().and_then(|s| ProvisionerService::connect_to_server(s).ok());

        // Provision container
        let is_remote = is_remote_server(target_server.as_ref().map(|s| s.url.as_str()).unwrap_or("local"));
        let exposed_port = if is_remote { Some(port) } else { None };
        let host_ip: Option<&str> = None;
        match provisioner.create_database_sni(
            target_docker.as_ref(),
            new_db_id,
            &source_db_type,
            &username,
            &password,
            source.database_name.as_deref().unwrap_or(&req.name),
            source.cpu_limit,
            source.memory_limit_mb as i64,
            is_remote,
            exposed_port,
            host_ip,
        ).await {
            Ok(result) => {
                // Generate TLS cert
                let fqdn = crate::utils::subdomain::subdomain_fqdn(&subdomain, &config.platform_domain);
                let cert = match tls_service.generate_cert_for_subdomain(&fqdn) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("TLS cert failed for clone {new_db_id}: {e}");
                        let _ = DatabaseRepository::update_status(&pool, new_db_id, &DbStatus::Error).await;
                        return;
                    }
                };

                let backend_address = compute_backend_address(new_db_id, &source_db_type, target_server.as_ref(), result.exposed_port);
                let _ = traefik_service.generate_sni_config(&new_db_id.to_string(), &source_db_type, &fqdn, &cert.cert_pem, &cert.key_pem, &backend_address);

                let _ = DatabaseRepository::update_provisioned(&pool, new_db_id, &result.container_id, &result.network_id, Some(&cert.cert_pem)).await;

                // Wait for container to be healthy before restore
                let docker_ref = target_docker.as_ref().unwrap_or(provisioner.fallback_docker());
                let _ = ProvisionerService::wait_for_healthy(docker_ref, &result.container_id, 60).await;

                // Restore backup
                let restore_result = match source_db_type {
                    DbType::Postgresql => provisioner.restore_postgres(target_docker.as_ref(), &result.container_id, &username, &password, source.database_name.as_deref().unwrap_or(&req.name), &backup_path).await,
                    DbType::Mariadb => provisioner.restore_mariadb(target_docker.as_ref(), &result.container_id, &username, &password, source.database_name.as_deref().unwrap_or(&req.name), &backup_path).await,
                    DbType::Redis => provisioner.restore_redis(target_docker.as_ref(), &result.container_id, &backup_path).await,
                };

                if let Err(e) = restore_result {
                    tracing::error!("Restore failed for clone {new_db_id}: {e}");
                }

                // Record billing
                let _ = BillingRepository::record_usage_event(&pool, new_db_id, "start", user_id, &req.name, source.plan_template_id).await;

                let _ = event_tx.send(DbEvent {
                    user_id,
                    database_id: new_db_id,
                    event_type: "created".to_string(),
                    status: Some(DbStatus::Running),
                });

                tracing::info!("Clone {new_db_id} created from backup {backup_id}");
            }
            Err(e) => {
                tracing::error!("Failed to provision clone {new_db_id}: {e}");
                let _ = DatabaseRepository::update_status(&pool, new_db_id, &DbStatus::Error).await;
            }
        }
    });

    Ok(Json(serde_json::json!({
        "id": new_db_id,
        "name": clone_name,
        "status": "provisioning",
    })))
}
