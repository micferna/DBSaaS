use std::sync::Arc;

use chrono::Datelike;
use axum::{
    Extension,
    middleware as axum_mw,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use axum::extract::DefaultBodyLimit;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use axum::http::{Method, header};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dbsaas_api::{
    config::Config,
    handlers::{admin, alerts, audit, auth, billing, databases, docker_servers, metrics, migrations, private_networks},
    middleware::{admin::admin_middleware, auth::auth_middleware, metrics::metrics_middleware, rate_limit::{create_rate_limiter, rate_limit_middleware}, security_headers::security_headers_middleware},
    repository::DatabaseRepository,
    services::{alert::AlertService, backup::BackupService, billing::BillingService, metrics as metrics_service, provisioner::ProvisionerService, tls::TlsService, traefik::TraefikService},
    utils::{docker::create_docker_client, port_pool::PortPool},
    AppState,
};

#[tokio::main]
async fn main() {
    // Install rustls CryptoProvider (required when multiple crates depend on rustls)
    let _ = rustls::crypto::ring::default_provider().install_default();


    // Load .env
    dotenvy::dotenv().ok();

    // Init tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "dbsaas_api=debug,tower_http=debug".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    // Database pool
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    tracing::info!("Running platform migrations...");
    run_migrations(&db).await;

    // Docker client
    let docker = create_docker_client(&config.docker_host).expect("Failed to connect to Docker");

    // Services
    let tls_service = Arc::new(TlsService::new(config.tls_ca_dir.clone()));
    tls_service.init_ca().expect("Failed to init CA");

    let provisioner = Arc::new(ProvisionerService::new(docker));

    // Ensure sb-proxy network exists for SNI routing
    match provisioner.ensure_proxy_network(None).await {
        Ok(id) => tracing::info!("Proxy network sb-proxy ready (id: {id})"),
        Err(e) => tracing::warn!("Could not ensure sb-proxy network: {e}"),
    }

    let traefik_service = Arc::new(TraefikService::new(config.traefik_dynamic_dir.clone()));

    // Port pool
    let port_pool = Arc::new(PortPool::new(config.port_range_start, config.port_range_end));
    let allocated_ports: Vec<i32> = DatabaseRepository::get_allocated_ports(&db)
        .await
        .unwrap_or_default();
    port_pool.load_allocated(allocated_ports);

    let (event_tx, _) = tokio::sync::broadcast::channel::<dbsaas_api::models::DbEvent>(256);

    // Init Prometheus metrics
    metrics_service::init_metrics();

    let state = AppState {
        db,
        registration_enabled: Arc::new(tokio::sync::RwLock::new(config.registration_enabled)),
        config: Arc::new(config.clone()),
        provisioner,
        tls_service,
        traefik_service,
        port_pool,
        event_tx,
        maintenance_mode: Arc::new(tokio::sync::RwLock::new(false)),
    };

    // Routes
    // Rate limiter: 5 requests/second for auth endpoints
    let auth_rate_limiter = create_rate_limiter(5);

    let public_routes = Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .layer(axum_mw::from_fn(rate_limit_middleware))
        .layer(Extension(auth_rate_limiter))
        .route("/api/health", get(health_check))
        .route("/api/stripe/webhook", post(billing::stripe_webhook))
        .route("/api/public/plans", get(billing::public_list_plans))
        .route("/api/metrics", get(metrics::prometheus_metrics));

    let user_routes = Router::new()
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/api-key", post(auth::generate_api_key_handler))
        // Audit logs
        .route("/api/audit-logs", get(audit::list_user_audit_logs))
        // Alerts
        .route("/api/alerts", get(alerts::list_alerts))
        .route("/api/alerts", post(alerts::create_alert))
        .route("/api/alerts/history", get(alerts::list_history))
        .route("/api/alerts/{id}", put(alerts::update_alert))
        .route("/api/alerts/{id}", delete(alerts::delete_alert))
        // Favorites (before {id} routes)
        .route("/api/databases/favorites", get(databases::list_favorites))
        .route("/api/databases/events", get(databases::database_events))
        .route("/api/databases", get(databases::list_databases))
        .route("/api/databases", post(databases::create_database))
        .route("/api/databases/bundle", post(databases::create_bundle))
        .route("/api/databases/ca-cert", get(databases::get_ca_cert))
        .route("/api/databases/{id}", get(databases::get_database))
        .route("/api/databases/{id}", delete(databases::delete_database))
        .route(
            "/api/databases/{id}/stats",
            get(databases::database_stats),
        )
        .route(
            "/api/databases/{id}/action",
            post(databases::container_action),
        )
        .route(
            "/api/databases/{id}/users",
            get(databases::list_database_users),
        )
        .route(
            "/api/databases/{id}/users",
            post(databases::create_database_user),
        )
        .route(
            "/api/databases/{db_id}/users/{user_id}",
            delete(databases::delete_database_user),
        )
        .route(
            "/api/databases/{db_id}/users/{user_id}/rotate-password",
            post(databases::rotate_user_password),
        )
        .route(
            "/api/databases/{id}/rotate-password",
            post(databases::rotate_owner_password),
        )
        .route(
            "/api/databases/{id}/backups",
            get(databases::list_backups),
        )
        .route(
            "/api/databases/{id}/backups",
            post(databases::create_backup),
        )
        .route(
            "/api/databases/{db_id}/backups/{backup_id}",
            delete(databases::delete_backup),
        )
        // Backup schedule
        .route("/api/databases/{id}/backup-schedule", post(databases::create_backup_schedule))
        .route("/api/databases/{id}/backup-schedule", get(databases::get_backup_schedule))
        .route("/api/databases/{id}/backup-schedule", put(databases::update_backup_schedule))
        .route("/api/databases/{id}/backup-schedule", delete(databases::delete_backup_schedule))
        // Export
        .route("/api/databases/{id}/export", post(databases::export_database))
        .route("/api/databases/{id}/export/{filename}", get(databases::download_export))
        // Scale / Rename / Clone
        .route("/api/databases/{id}/scale", put(databases::scale_database))
        .route("/api/databases/{id}/rename", put(databases::rename_database))
        .route("/api/databases/{db_id}/clone/{backup_id}", post(databases::clone_database))
        // Favorites
        .route("/api/databases/{id}/favorite", post(databases::add_favorite))
        .route("/api/databases/{id}/favorite", delete(databases::remove_favorite))
        .route(
            "/api/databases/{id}/migrations",
            post(migrations::upload_migration),
        )
        .route(
            "/api/databases/{id}/migrations",
            get(migrations::list_migrations),
        )
        // Network routes
        .route("/api/networks", post(private_networks::create_network))
        .route("/api/networks", get(private_networks::list_networks))
        .route("/api/networks/{id}", get(private_networks::get_network))
        .route("/api/networks/{id}", delete(private_networks::delete_network))
        .route("/api/networks/{id}/attach", post(private_networks::attach_database))
        .route("/api/networks/{id}/detach", post(private_networks::detach_database))
        // Peering routes
        .route("/api/peerings", get(private_networks::list_peerings))
        .route("/api/peerings", post(private_networks::create_peering))
        .route("/api/peerings/{id}", get(private_networks::get_peering))
        .route("/api/peerings/{id}", delete(private_networks::delete_peering))
        .route("/api/peerings/{id}/rules", post(private_networks::create_firewall_rule))
        .route("/api/peerings/{peering_id}/rules/{rule_id}", delete(private_networks::delete_firewall_rule))
        // Servers
        .route("/api/servers", get(databases::list_available_servers))
        // Billing routes
        .route("/api/plans", get(billing::list_plans))
        .route("/api/billing/periods", get(billing::billing_periods))
        .route("/api/billing/current", get(billing::billing_current))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let admin_routes = Router::new()
        .route("/api/admin/stats", get(admin::admin_stats))
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/users/{id}/role", put(admin::update_user_role))
        .route("/api/admin/users/{id}", delete(admin::delete_user))
        .route("/api/admin/databases", get(admin::list_all_databases))
        .route(
            "/api/admin/databases/{id}",
            delete(admin::force_delete_database),
        )
        .route("/api/admin/invitations", get(admin::list_invitations))
        .route("/api/admin/invitations", post(admin::create_invitation))
        .route(
            "/api/admin/invitations/{id}",
            delete(admin::delete_invitation),
        )
        // Admin billing routes
        .route("/api/admin/plans", get(billing::admin_list_plans))
        .route("/api/admin/plans", post(billing::admin_create_plan))
        .route("/api/admin/plans/{id}", put(billing::admin_update_plan))
        .route("/api/admin/plans/{id}", delete(billing::admin_delete_plan))
        .route("/api/admin/billing/overview", get(billing::admin_billing_overview))
        .route("/api/admin/billing/generate", post(billing::admin_generate_billing))
        // Migrate database to SNI routing
        .route("/api/admin/databases/{id}/migrate-sni", post(admin::migrate_to_sni))
        // Settings
        .route("/api/admin/settings/registration", put(admin::toggle_registration))
        // Docker server routes
        .route("/api/admin/servers", get(docker_servers::list_servers))
        .route("/api/admin/servers", post(docker_servers::create_server))
        .route("/api/admin/servers/status", get(docker_servers::servers_status))
        .route("/api/admin/servers/{id}", put(docker_servers::update_server))
        .route("/api/admin/servers/{id}", delete(docker_servers::delete_server))
        .route("/api/admin/servers/{id}/status", get(docker_servers::server_status))
        .route("/api/admin/servers/{id}/containers", get(docker_servers::server_containers))
        .route("/api/admin/servers/{id}/resources", get(docker_servers::server_resources))
        // Admin network routes
        .route("/api/admin/networks", get(private_networks::admin_list_networks))
        .route("/api/admin/peerings", get(private_networks::admin_list_peerings))
        // Admin audit, health, maintenance, user resources
        .route("/api/admin/audit-logs", get(audit::list_admin_audit_logs))
        .route("/api/admin/health", get(admin::system_health))
        .route("/api/admin/settings/maintenance", put(admin::toggle_maintenance))
        .route("/api/admin/users/{id}/resources", get(admin::user_resources))
        .layer(axum_mw::from_fn(admin_middleware))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let methods = vec![
        Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS, Method::PATCH,
    ];
    let headers = vec![
        header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT, header::ORIGIN,
    ];

    let cors = if let Some(origins) = &state.config.cors_origins {
        let allowed: Vec<axum::http::HeaderValue> = origins
            .iter()
            .filter_map(|o: &String| o.parse::<axum::http::HeaderValue>().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods(methods)
            .allow_headers(headers)
    } else {
        tracing::warn!("CORS_ORIGINS not set — defaulting to localhost only");
        let defaults: Vec<axum::http::HeaderValue> = vec![
            "http://localhost:3000".parse().unwrap(),
            "http://localhost:3003".parse().unwrap(),
        ];
        CorsLayer::new()
            .allow_origin(defaults)
            .allow_methods(methods)
            .allow_headers(headers)
    };

    let app = Router::new()
        .merge(public_routes)
        .merge(user_routes)
        .merge(admin_routes)
        .layer(cors)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB max request body
        .layer(axum_mw::from_fn(security_headers_middleware))
        .layer(axum_mw::from_fn(metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    // Spawn background billing task
    {
        let billing_pool = state.db.clone();
        let billing_stripe_key = state.config.stripe_secret_key.clone();
        tokio::spawn(async move {
            let mut last_run_month: Option<u32> = None;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                let now = chrono::Utc::now();
                // Run on the 1st of each month, once
                if now.day() == 1 {
                    let current_month = now.month();
                    if last_run_month != Some(current_month) {
                        tracing::info!("Running monthly billing generation...");
                        match BillingService::run_monthly_billing(
                            &billing_pool,
                            billing_stripe_key.as_deref(),
                        )
                        .await
                        {
                            Ok(count) => {
                                tracing::info!("Monthly billing: {count} invoices created");
                                last_run_month = Some(current_month);
                            }
                            Err(e) => {
                                tracing::error!("Monthly billing failed: {e}");
                            }
                        }
                    }
                }
            }
        });
    }

    // Spawn metrics refresh task (every 60s)
    {
        let pool = state.db.clone();
        tokio::spawn(async move {
            loop {
                metrics_service::refresh_platform_metrics(&pool).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });
    }

    // Spawn backup scheduler task (every 5 min)
    {
        let pool = state.db.clone();
        let provisioner = state.provisioner.clone();
        let backup_dir = state.config.backup_dir.clone();
        let enc_key = state.config.encryption_key.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                BackupService::run_scheduled_backups(&pool, &provisioner, &backup_dir, &enc_key).await;
            }
        });
    }

    // Spawn alert checker task (every 30s)
    {
        let pool = state.db.clone();
        let provisioner = state.provisioner.clone();
        let config = state.config.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                AlertService::check_container_health(&pool, &provisioner, &config).await;
            }
        });
    }

    let addr = format!("{}:{}", state.config.host, state.config.port);
    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}

async fn health_check() -> &'static str {
    "OK"
}

async fn run_migrations(pool: &sqlx::PgPool) {
    let migration_files = [
        include_str!("../migrations/001_create_users.sql"),
        include_str!("../migrations/002_create_databases.sql"),
        include_str!("../migrations/003_create_invitations.sql"),
        include_str!("../migrations/004_create_migrations.sql"),
        include_str!("../migrations/005_add_bundle_id.sql"),
        include_str!("../migrations/006_create_database_users.sql"),
        include_str!("../migrations/007_mariadb_backups_tls.sql"),
        include_str!("../migrations/008_billing_system.sql"),
        include_str!("../migrations/009_docker_servers_and_seed_plans.sql"),
        include_str!("../migrations/010_server_type_and_placement.sql"),
        include_str!("../migrations/011_deduplicate_plans.sql"),
        include_str!("../migrations/012_private_networks.sql"),
        include_str!("../migrations/013_network_subnet.sql"),
        include_str!("../migrations/014_fix_billing_fk.sql"),
        include_str!("../migrations/015_add_subdomain.sql"),
        include_str!("../migrations/016_network_peering.sql"),
        include_str!("../migrations/017_backup_schedules.sql"),
        include_str!("../migrations/018_audit_logs.sql"),
        include_str!("../migrations/019_alerts.sql"),
        include_str!("../migrations/020_user_favorites.sql"),
        include_str!("../migrations/021_platform_settings.sql"),
    ];

    for (i, sql_file) in migration_files.iter().enumerate() {
        let statements: Vec<&str> = sql_file
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut failed = false;
        for stmt in statements {
            match sqlx::query(stmt).execute(pool).await {
                Ok(_) => {}
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("already exists") || err_str.contains("duplicate") {
                        tracing::debug!("Migration {} (statement already applied)", i + 1);
                    } else {
                        tracing::error!("Migration {} failed: {}", i + 1, e);
                        failed = true;
                        break;
                    }
                }
            }
        }
        if !failed {
            tracing::info!("Migration {} applied", i + 1);
        }
    }
}
