use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "db_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Postgresql,
    Redis,
    Mariadb,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "db_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DbStatus {
    Provisioning,
    Running,
    Stopped,
    Error,
    Deleting,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "db_permission", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DbPermission {
    Admin,
    ReadWrite,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DatabaseInstance {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub db_type: DbType,
    pub status: DbStatus,
    pub container_id: Option<String>,
    pub network_id: Option<String>,
    pub host: String,
    pub port: i32,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_encrypted: String,
    pub database_name: Option<String>,
    pub tls_cert: Option<String>,
    pub cpu_limit: f64,
    pub memory_limit_mb: i32,
    pub bundle_id: Option<Uuid>,
    pub tls_mode: String,
    pub plan_template_id: Option<Uuid>,
    pub subdomain: String,
    pub routing_mode: String,
    pub docker_server_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DatabaseInstance {
    pub fn db_type_str(&self) -> &str {
        match self.db_type {
            DbType::Postgresql => "pg",
            DbType::Redis => "redis",
            DbType::Mariadb => "mariadb",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Bundle {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub network_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DatabaseUser {
    pub id: Uuid,
    pub database_id: Uuid,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_encrypted: String,
    pub permission: DbPermission,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BackupRecord {
    pub id: Uuid,
    pub database_id: Uuid,
    pub filename: String,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbEvent {
    pub user_id: Uuid,
    pub database_id: Uuid,
    pub event_type: String,
    pub status: Option<DbStatus>,
}

fn validate_db_name(name: &str) -> Result<(), validator::ValidationError> {
    if name.is_empty() || name.len() > 63 {
        return Err(validator::ValidationError::new("invalid_length"));
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Err(validator::ValidationError::new("must_start_with_letter"));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(validator::ValidationError::new("invalid_characters"));
    }
    Ok(())
}

fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    if username.is_empty() || username.len() > 63 {
        return Err(validator::ValidationError::new("invalid_length"));
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(validator::ValidationError::new("invalid_characters"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDatabaseRequest {
    #[validate(custom(function = "validate_db_name"))]
    pub name: String,
    pub db_type: DbType,
    pub plan_template_id: Option<Uuid>,
    pub cpu_limit: Option<f64>,
    pub memory_limit_mb: Option<i32>,
    /// SSL mode: "verify-ca" (recommended, requires CA cert) or "require" (encrypted, no cert needed).
    /// Defaults to "require" if not specified.
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    pub server_id: Option<Uuid>,
}

fn default_ssl_mode() -> String {
    "require".to_string()
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBundleRequest {
    #[validate(custom(function = "validate_db_name"))]
    pub name: String,
    pub plan_template_id: Option<Uuid>,
    pub cpu_limit: Option<f64>,
    pub memory_limit_mb: Option<i32>,
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    pub server_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BundleResponse {
    pub id: Uuid,
    pub name: String,
    pub postgresql: DatabaseResponse,
    pub redis: DatabaseResponse,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDatabaseUserRequest {
    #[validate(custom(function = "validate_username"))]
    pub username: String,
    pub permission: DbPermission,
}

#[derive(Debug, Serialize)]
pub struct DatabaseUserResponse {
    pub id: Uuid,
    pub database_id: Uuid,
    pub username: String,
    pub password: String,
    pub permission: DbPermission,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseUserListItem {
    pub id: Uuid,
    pub database_id: Uuid,
    pub username: String,
    pub permission: DbPermission,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
}

#[derive(Debug, Deserialize)]
pub struct ContainerActionRequest {
    pub action: ContainerAction,
}

#[derive(Debug, Serialize)]
pub struct DatabaseResponse {
    pub id: Uuid,
    pub name: String,
    pub db_type: DbType,
    pub status: DbStatus,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub database_name: Option<String>,
    pub connection_url: String,
    pub tls_enabled: bool,
    /// "verify-ca" (full TLS with CA cert) or "require" (encrypted, no cert needed)
    pub ssl_mode: String,
    pub cpu_limit: f64,
    pub memory_limit_mb: i32,
    pub bundle_id: Option<Uuid>,
    pub plan_template_id: Option<Uuid>,
    pub subdomain: Option<String>,
    pub routing_mode: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MigrationRecord {
    pub id: Uuid,
    pub database_id: Uuid,
    pub filename: String,
    pub checksum: String,
    pub applied_at: DateTime<Utc>,
}
