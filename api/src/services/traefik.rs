use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::models::DbType;

pub struct TraefikService {
    dynamic_dir: String,
}

impl TraefikService {
    pub fn new(dynamic_dir: String) -> Self {
        Self { dynamic_dir }
    }

    pub fn generate_config(
        &self,
        db_id: &str,
        db_type: &DbType,
        port: u16,
        tls_mode: &str,
        cert_pem: Option<&str>,
        key_pem: Option<&str>,
    ) -> AppResult<()> {
        std::fs::create_dir_all(&self.dynamic_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create traefik dir: {e}")))?;

        let internal_port = match db_type {
            DbType::Postgresql => 5432,
            DbType::Redis => 6379,
            DbType::Mariadb => 3306,
        };

        let protocol = match db_type {
            DbType::Postgresql => "postgresql",
            DbType::Redis => "redis",
            DbType::Mariadb => "mariadb",
        };

        let tls_enabled = tls_mode == "enabled";

        let tls_section = if tls_enabled {
            format!(
                r#"  [tcp.routers.{db_id}.tls]

[[tls.certificates]]
  certFile = "/etc/traefik/dynamic/certs/{db_id}.crt"
  keyFile = "/etc/traefik/dynamic/certs/{db_id}.key"
"#
            )
        } else {
            String::new()
        };

        let config = format!(
            r#"# Auto-generated config for {protocol} instance {db_id}

[tcp.routers.{db_id}]
  rule = "HostSNI(`*`)"
  entryPoints = ["{protocol}-{port}"]
  service = "{db_id}"
{tls_section}
[tcp.services.{db_id}.loadBalancer]
  terminationDelay = 5000
  [[tcp.services.{db_id}.loadBalancer.servers]]
    address = "sb-{db_id}:{internal_port}"
"#
        );

        let config_path = Path::new(&self.dynamic_dir).join(format!("{db_id}.toml"));
        std::fs::write(&config_path, config)
            .map_err(|e| AppError::Internal(format!("Failed to write traefik config: {e}")))?;

        // Write certs only if TLS enabled
        if tls_enabled {
            if let (Some(cert), Some(key)) = (cert_pem, key_pem) {
                let certs_dir = Path::new(&self.dynamic_dir).join("certs");
                std::fs::create_dir_all(&certs_dir)
                    .map_err(|e| AppError::Internal(format!("Failed to create certs dir: {e}")))?;
                std::fs::write(certs_dir.join(format!("{db_id}.crt")), cert)
                    .map_err(|e| AppError::Internal(format!("Failed to write cert: {e}")))?;
                std::fs::write(certs_dir.join(format!("{db_id}.key")), key)
                    .map_err(|e| AppError::Internal(format!("Failed to write key: {e}")))?;
            }
        }

        Ok(())
    }

    /// Generate SNI-based Traefik config for a database.
    /// Routes TCP traffic based on HostSNI matching the subdomain FQDN.
    /// TLS is terminated by Traefik; container receives plain TCP.
    ///
    /// `backend_address` is the address Traefik will proxy to:
    /// - Local: `"sb-{db_id}:5432"` (Docker DNS via shared proxy network)
    /// - Remote: `"192.168.1.12:PORT"` (server IP + exposed port)
    pub fn generate_sni_config(
        &self,
        db_id: &str,
        db_type: &DbType,
        subdomain_fqdn: &str,
        cert_pem: &str,
        key_pem: &str,
        backend_address: &str,
    ) -> AppResult<()> {
        std::fs::create_dir_all(&self.dynamic_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create traefik dir: {e}")))?;

        let entrypoint = match db_type {
            DbType::Postgresql => "postgres",
            DbType::Redis => "redis",
            DbType::Mariadb => "mariadb",
        };

        let config = format!(
            r#"# Auto-generated SNI config for {db_id}

[tcp.routers.{db_id}]
  rule = "HostSNI(`{subdomain_fqdn}`)"
  entryPoints = ["{entrypoint}"]
  service = "{db_id}"
  [tcp.routers.{db_id}.tls]

[tcp.services.{db_id}.loadBalancer]
  terminationDelay = 5000
  [[tcp.services.{db_id}.loadBalancer.servers]]
    address = "{backend_address}"

[[tls.certificates]]
  certFile = "/etc/traefik/dynamic/certs/{db_id}.crt"
  keyFile = "/etc/traefik/dynamic/certs/{db_id}.key"
"#
        );

        let config_path = Path::new(&self.dynamic_dir).join(format!("{db_id}.toml"));
        std::fs::write(&config_path, config)
            .map_err(|e| AppError::Internal(format!("Failed to write traefik config: {e}")))?;

        // Write certs
        let certs_dir = Path::new(&self.dynamic_dir).join("certs");
        std::fs::create_dir_all(&certs_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create certs dir: {e}")))?;
        std::fs::write(certs_dir.join(format!("{db_id}.crt")), cert_pem)
            .map_err(|e| AppError::Internal(format!("Failed to write cert: {e}")))?;
        std::fs::write(certs_dir.join(format!("{db_id}.key")), key_pem)
            .map_err(|e| AppError::Internal(format!("Failed to write key: {e}")))?;

        Ok(())
    }

    pub fn remove_config(&self, db_id: &str) -> AppResult<()> {
        let config_path = Path::new(&self.dynamic_dir).join(format!("{db_id}.toml"));
        if config_path.exists() {
            std::fs::remove_file(&config_path)
                .map_err(|e| AppError::Internal(format!("Failed to remove traefik config: {e}")))?;
        }

        let certs_dir = Path::new(&self.dynamic_dir).join("certs");
        let _ = std::fs::remove_file(certs_dir.join(format!("{db_id}.crt")));
        let _ = std::fs::remove_file(certs_dir.join(format!("{db_id}.key")));

        Ok(())
    }
}
