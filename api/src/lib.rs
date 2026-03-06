pub mod config;
pub mod error;
pub mod extract;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repository;
pub mod services;
pub mod utils;

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use sqlx::PgPool;

use crate::config::Config;
use crate::models::DbEvent;
use crate::services::provisioner::ProvisionerService;
use crate::services::tls::TlsService;
use crate::services::traefik::TraefikService;
use crate::utils::port_pool::PortPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub provisioner: Arc<ProvisionerService>,
    pub tls_service: Arc<TlsService>,
    pub traefik_service: Arc<TraefikService>,
    pub port_pool: Arc<PortPool>,
    /// Runtime-mutable settings
    pub registration_enabled: Arc<RwLock<bool>>,
    /// SSE broadcast channel for real-time database events
    pub event_tx: broadcast::Sender<DbEvent>,
    /// Maintenance mode flag
    pub maintenance_mode: Arc<RwLock<bool>>,
}
