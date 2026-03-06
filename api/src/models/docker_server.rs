use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DockerServer {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    #[serde(skip_serializing)]
    pub tls_ca: Option<String>,
    #[serde(skip_serializing)]
    pub tls_cert: Option<String>,
    #[serde(skip_serializing)]
    pub tls_key: Option<String>,
    pub max_containers: i32,
    pub active: bool,
    pub region: Option<String>,
    pub notes: Option<String>,
    pub server_type: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDockerServerRequest {
    pub name: String,
    pub url: String,
    pub tls_ca: Option<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub max_containers: Option<i32>,
    pub region: Option<String>,
    pub notes: Option<String>,
    pub server_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDockerServerRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub tls_ca: Option<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub max_containers: Option<i32>,
    pub active: Option<bool>,
    pub region: Option<String>,
    pub notes: Option<String>,
    pub server_type: Option<String>,
}

/// Live status info returned to the admin
#[derive(Debug, Serialize)]
pub struct DockerServerStatus {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub region: Option<String>,
    pub active: bool,
    pub server_type: String,
    pub max_containers: i32,
    pub online: bool,
    pub containers_running: Option<i64>,
    pub containers_total: Option<i64>,
    pub cpu_count: Option<i64>,
    pub memory_bytes: Option<i64>,
    pub docker_version: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}
