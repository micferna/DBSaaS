use bollard::models::{ContainerCreateBody, ContainerUpdateBody, HealthConfig, HealthStatusEnum, HostConfig, NetworkConnectRequest, NetworkCreateRequest, NetworkDisconnectRequest, PortBinding, RestartPolicy, RestartPolicyNameEnum};
use bollard::query_parameters::{CreateContainerOptions, CreateImageOptions, DownloadFromContainerOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions, RestartContainerOptions, StatsOptions, UploadToContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{DbPermission, DbType, DockerServer};

/// Firewall rule tuple: (src_subnet, dst_subnet, port, protocol, action)
pub type FirewallRule = (String, String, Option<i32>, Option<String>, String);

/// ProvisionerService manages Docker container lifecycle.
/// It holds a fallback Docker client (local) but all public methods accept
/// an optional `&Docker` to target a specific remote server.
pub struct ProvisionerService {
    fallback_docker: Docker,
}

#[derive(Debug)]
pub struct ProvisionResult {
    pub container_id: String,
    pub network_id: String,
    pub exposed_port: Option<u16>,
}

impl ProvisionerService {
    pub fn new(docker: Docker) -> Self {
        Self { fallback_docker: docker }
    }

    /// Create a Docker client for a given server.
    /// Local servers use unix socket. Remote servers require mTLS certificates.
    pub fn connect_to_server(server: &DockerServer) -> AppResult<Docker> {
        if server.url == "local" || server.url == "unix:///var/run/docker.sock" {
            Docker::connect_with_local_defaults()
                .map_err(|e| AppError::Internal(format!("Docker connection failed: {e}")))
        } else {
            // Remote server — require TLS certificates
            let ca = server.tls_ca.as_deref().ok_or_else(|| {
                AppError::BadRequest("TLS certificates required for remote Docker connections (missing CA)".to_string())
            })?;
            let cert = server.tls_cert.as_deref().ok_or_else(|| {
                AppError::BadRequest("TLS certificates required for remote Docker connections (missing cert)".to_string())
            })?;
            let key = server.tls_key.as_deref().ok_or_else(|| {
                AppError::BadRequest("TLS certificates required for remote Docker connections (missing key)".to_string())
            })?;

            // Persist PEM files to disk — bollard reads them lazily during TLS handshake,
            // so temp files would be deleted before they're read.
            let pem_dir = Self::pem_dir_for_server(server.id)?;
            let ca_path = pem_dir.join("ca.pem");
            let cert_path = pem_dir.join("cert.pem");
            let key_path = pem_dir.join("key.pem");

            Self::write_pem_file(&ca_path, ca)?;
            Self::write_pem_file(&cert_path, cert)?;
            Self::write_pem_file(&key_path, key)?;

            Docker::connect_with_ssl(
                &server.url,
                &key_path,
                &cert_path,
                &ca_path,
                120,
                bollard::API_DEFAULT_VERSION,
            )
            .map_err(|e| AppError::Internal(format!("Docker TLS connection failed: {e}")))
        }
    }

    /// Get (and create) a directory for persisted PEM files for a given server
    fn pem_dir_for_server(server_id: Uuid) -> AppResult<std::path::PathBuf> {
        let dir = std::path::PathBuf::from(format!("/tmp/dbsaas-tls/{}", server_id));
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Internal(format!("Failed to create PEM directory: {e}")))?;
        Ok(dir)
    }

    /// Write PEM content to a persistent file
    fn write_pem_file(path: &std::path::Path, data: &str) -> AppResult<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)
            .map_err(|e| AppError::Internal(format!("Failed to create PEM file: {e}")))?;
        file.write_all(data.as_bytes())
            .map_err(|e| AppError::Internal(format!("Failed to write PEM data: {e}")))?;
        // Restrict permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| AppError::Internal(format!("Failed to set PEM permissions: {e}")))?;
        }
        Ok(())
    }

    /// Get the Docker client to use — target if provided, otherwise fallback
    fn docker<'a>(&'a self, target: Option<&'a Docker>) -> &'a Docker {
        target.unwrap_or(&self.fallback_docker)
    }

    /// Get a reference to the fallback (local) Docker client.
    pub fn fallback_docker(&self) -> &Docker {
        &self.fallback_docker
    }

    /// Ensure a Docker image is available locally, pulling it if necessary.
    async fn ensure_image(docker: &Docker, image: &str, tag: &str) -> AppResult<()> {
        let full = format!("{image}:{tag}");
        // Check if image already exists
        if docker.inspect_image(&full).await.is_ok() {
            tracing::debug!("Image {full} already present");
            return Ok(());
        }

        tracing::info!("Pulling image {full}...");
        let options = CreateImageOptions {
            from_image: Some(image.to_string()),
            tag: Some(tag.to_string()),
            ..Default::default()
        };
        let mut stream = docker.create_image(Some(options), None, None);
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = &info.status {
                        tracing::debug!("Pull {full}: {status}");
                    }
                }
                Err(e) => {
                    return Err(AppError::Internal(format!("Failed to pull image {full}: {e}")));
                }
            }
        }
        tracing::info!("Image {full} pulled successfully");
        Ok(())
    }

    /// Get (image, tag) for a given database type
    fn image_for_db_type(db_type: &DbType) -> (&'static str, &'static str) {
        match db_type {
            DbType::Postgresql => ("postgres", "17-alpine"),
            DbType::Redis => ("redis", "8-alpine"),
            DbType::Mariadb => ("mariadb", "11-noble"),
        }
    }

    pub async fn create_database(
        &self,
        target_docker: Option<&Docker>,
        db_id: Uuid,
        db_type: &DbType,
        username: &str,
        password: &str,
        db_name: &str,
        port: u16,
        cpu_limit: f64,
        memory_limit_mb: i64,
    ) -> AppResult<ProvisionResult> {
        let docker = self.docker(target_docker);
        let network_name = format!("sb-net-{db_id}");
        let container_name = format!("sb-{db_id}");

        // Ensure the image is pulled on the target server
        let (image, tag) = Self::image_for_db_type(db_type);
        Self::ensure_image(docker, image, tag).await?;

        let network_id = Self::create_network(docker, &network_name).await?;

        let container_id = match db_type {
            DbType::Postgresql => {
                Self::create_postgres_container(
                    docker, &container_name, &network_name, username, password, db_name,
                    port, cpu_limit, memory_limit_mb, None,
                ).await?
            }
            DbType::Redis => {
                Self::create_redis_container(
                    docker, &container_name, &network_name, password,
                    port, cpu_limit, memory_limit_mb, None,
                ).await?
            }
            DbType::Mariadb => {
                Self::create_mariadb_container(
                    docker, &container_name, &network_name, username, password, db_name,
                    port, cpu_limit, memory_limit_mb, None,
                ).await?
            }
        };

        docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await?;

        tracing::info!("Provisioned {db_type:?} container {container_id} on port {port}");

        Ok(ProvisionResult { container_id, network_id, exposed_port: None })
    }

    /// Create a database container in SNI mode.
    /// - Local (is_remote=false): no port bindings, connected to sb-proxy network.
    /// - Remote (is_remote=true): port exposed on host, NOT connected to proxy network
    ///   (Traefik routes via server IP instead of Docker DNS).
    pub async fn create_database_sni(
        &self,
        target_docker: Option<&Docker>,
        db_id: Uuid,
        db_type: &DbType,
        username: &str,
        password: &str,
        db_name: &str,
        cpu_limit: f64,
        memory_limit_mb: i64,
        is_remote: bool,
        exposed_port: Option<u16>,
        host_ip: Option<&str>,
    ) -> AppResult<ProvisionResult> {
        let docker = self.docker(target_docker);
        let network_name = format!("sb-net-{db_id}");
        let container_name = format!("sb-{db_id}");

        let (image, tag) = Self::image_for_db_type(db_type);
        Self::ensure_image(docker, image, tag).await?;

        // Remote: network must NOT be internal (so port bindings work)
        let network_id = Self::create_network_with_opts(docker, &network_name, !is_remote).await?;

        let container_id = if let (true, Some(port)) = (is_remote, exposed_port) {
            // Remote: use host port binding so Traefik can reach via IP:port
            match db_type {
                DbType::Postgresql => {
                    Self::create_postgres_container(
                        docker, &container_name, &network_name, username, password, db_name,
                        port, cpu_limit, memory_limit_mb, host_ip,
                    ).await?
                }
                DbType::Redis => {
                    Self::create_redis_container(
                        docker, &container_name, &network_name, password,
                        port, cpu_limit, memory_limit_mb, host_ip,
                    ).await?
                }
                DbType::Mariadb => {
                    Self::create_mariadb_container(
                        docker, &container_name, &network_name, username, password, db_name,
                        port, cpu_limit, memory_limit_mb, host_ip,
                    ).await?
                }
            }
        } else {
            // Local: no port bindings, will connect to proxy network
            match db_type {
                DbType::Postgresql => {
                    Self::create_postgres_container_sni(
                        docker, &container_name, &network_name, username, password, db_name,
                        cpu_limit, memory_limit_mb,
                    ).await?
                }
                DbType::Redis => {
                    Self::create_redis_container_sni(
                        docker, &container_name, &network_name, password,
                        cpu_limit, memory_limit_mb,
                    ).await?
                }
                DbType::Mariadb => {
                    Self::create_mariadb_container_sni(
                        docker, &container_name, &network_name, username, password, db_name,
                        cpu_limit, memory_limit_mb,
                    ).await?
                }
            }
        };

        docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await?;

        if !is_remote {
            // Local only: connect to proxy network so Traefik can reach by container name
            self.ensure_proxy_network(target_docker).await?;
            self.connect_to_proxy_network(target_docker, &container_id, "sb-proxy").await?;
        }

        tracing::info!("Provisioned {db_type:?} container {container_id} in SNI mode (remote={is_remote})");

        Ok(ProvisionResult { container_id, network_id, exposed_port })
    }

    async fn create_network(docker: &Docker, name: &str) -> AppResult<String> {
        Self::create_network_with_opts(docker, name, true).await
    }

    async fn create_network_with_opts(docker: &Docker, name: &str, internal: bool) -> AppResult<String> {
        let options = NetworkCreateRequest {
            name: name.to_string(),
            internal: Some(internal),
            driver: Some("bridge".to_string()),
            ..Default::default()
        };
        let response = docker.create_network(options).await?;
        let id = response.id;
        if id.is_empty() {
            return Err(AppError::Internal("Network creation returned no ID".to_string()));
        }
        Ok(id)
    }

    /// Build host config for SNI mode — no port bindings (Traefik routes via shared entrypoint).
    fn build_host_config_sni(network: &str, cpu_limit: f64, memory_limit_mb: i64) -> HostConfig {
        HostConfig {
            network_mode: Some(network.to_string()),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                ..Default::default()
            }),
            nano_cpus: Some((cpu_limit * 1_000_000_000.0) as i64),
            memory: Some(memory_limit_mb * 1024 * 1024),
            memory_swap: Some(memory_limit_mb * 1024 * 1024),
            security_opt: Some(vec!["no-new-privileges:true".to_string()]),
            ..Default::default()
        }
    }

    /// Build a Docker healthcheck config for a given database type.
    fn healthcheck_for_db_type(db_type: &DbType, password: &str) -> HealthConfig {
        match db_type {
            DbType::Postgresql => HealthConfig {
                test: Some(vec![
                    "CMD-SHELL".to_string(),
                    "pg_isready -U $POSTGRES_USER || exit 1".to_string(),
                ]),
                interval: Some(10_000_000_000),  // 10s in nanoseconds
                timeout: Some(5_000_000_000),     // 5s
                retries: Some(3),
                start_period: Some(15_000_000_000), // 15s
                ..Default::default()
            },
            DbType::Redis => HealthConfig {
                test: Some(vec![
                    "CMD-SHELL".to_string(),
                    format!("redis-cli -a {} --no-auth-warning ping | grep PONG", password),
                ]),
                interval: Some(10_000_000_000),
                timeout: Some(5_000_000_000),
                retries: Some(3),
                start_period: Some(10_000_000_000),
                ..Default::default()
            },
            DbType::Mariadb => HealthConfig {
                test: Some(vec![
                    "CMD-SHELL".to_string(),
                    "healthcheck --connect --innodb_initialized".to_string(),
                ]),
                interval: Some(10_000_000_000),
                timeout: Some(5_000_000_000),
                retries: Some(3),
                start_period: Some(30_000_000_000),
                ..Default::default()
            },
        }
    }

    /// Wait for a container to become healthy.
    /// Falls back to checking `running` state if no healthcheck is configured.
    pub async fn wait_for_healthy(
        docker: &Docker,
        container_id: &str,
        timeout_secs: u64,
    ) -> AppResult<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(AppError::Internal(format!(
                    "Container {container_id} did not become healthy within {timeout_secs}s"
                )));
            }

            let info = docker.inspect_container(container_id, None).await?;
            if let Some(state) = &info.state {
                if let Some(health) = &state.health {
                    match health.status {
                        Some(HealthStatusEnum::HEALTHY) => return Ok(()),
                        Some(HealthStatusEnum::UNHEALTHY) => {
                            return Err(AppError::Internal(format!(
                                "Container {container_id} is unhealthy"
                            )));
                        }
                        _ => {} // STARTING, EMPTY, NONE — keep polling
                    }
                } else {
                    // No healthcheck configured — fall back to running check
                    if state.running == Some(true) {
                        return Ok(());
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    /// Connect a container to the sb-proxy network so Traefik can reach it.
    pub async fn connect_to_proxy_network(
        &self,
        target_docker: Option<&Docker>,
        container_id: &str,
        proxy_network: &str,
    ) -> AppResult<()> {
        let docker = self.docker(target_docker);
        docker
            .connect_network(
                proxy_network,
                NetworkConnectRequest {
                    container: container_id.to_string(),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    /// Ensure the sb-proxy network exists, create it if not.
    pub async fn ensure_proxy_network(&self, target_docker: Option<&Docker>) -> AppResult<String> {
        let docker = self.docker(target_docker);
        // Try to inspect existing network
        if let Ok(info) = docker.inspect_network("sb-proxy", None).await {
            if let Some(id) = info.id {
                return Ok(id);
            }
        }
        // Create it
        let options = NetworkCreateRequest {
            name: "sb-proxy".to_string(),
            driver: Some("bridge".to_string()),
            ..Default::default()
        };
        let response = docker.create_network(options).await?;
        Ok(response.id)
    }

    fn build_host_config(network: &str, port_internal: &str, port_external: u16, cpu_limit: f64, memory_limit_mb: i64, host_ip: Option<&str>) -> HostConfig {
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            port_internal.to_string(),
            Some(vec![PortBinding {
                host_ip: Some(host_ip.unwrap_or("127.0.0.1").to_string()),
                host_port: Some(port_external.to_string()),
            }]),
        );

        HostConfig {
            port_bindings: Some(port_bindings),
            network_mode: Some(network.to_string()),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                ..Default::default()
            }),
            nano_cpus: Some((cpu_limit * 1_000_000_000.0) as i64),
            memory: Some(memory_limit_mb * 1024 * 1024),
            memory_swap: Some(memory_limit_mb * 1024 * 1024),
            security_opt: Some(vec!["no-new-privileges:true".to_string()]),
            ..Default::default()
        }
    }

    async fn create_postgres_container(
        docker: &Docker, name: &str, network: &str, username: &str, password: &str,
        db_name: &str, port: u16, cpu_limit: f64, memory_limit_mb: i64, host_ip: Option<&str>,
    ) -> AppResult<String> {
        let env = [
            format!("POSTGRES_USER={username}"),
            format!("POSTGRES_PASSWORD={password}"),
            format!("POSTGRES_DB={db_name}"),
        ];
        let host_config = Self::build_host_config(network, "5432/tcp", port, cpu_limit, memory_limit_mb, host_ip);
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Postgresql, password);
        let config = ContainerCreateBody {
            image: Some("postgres:17-alpine".to_string()),
            env: Some(env.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    async fn create_redis_container(
        docker: &Docker, name: &str, network: &str, password: &str,
        port: u16, cpu_limit: f64, memory_limit_mb: i64, host_ip: Option<&str>,
    ) -> AppResult<String> {
        let host_config = Self::build_host_config(network, "6379/tcp", port, cpu_limit, memory_limit_mb, host_ip);
        let max_mem = format!("{memory_limit_mb}mb");
        let cmd: Vec<String> = vec![
            "redis-server".to_string(), "--requirepass".to_string(), password.to_string(),
            "--appendonly".to_string(), "yes".to_string(),
            "--maxmemory".to_string(), max_mem, "--maxmemory-policy".to_string(), "allkeys-lru".to_string(),
        ];
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Redis, password);
        let env_vars = [format!("REDIS_PASSWORD={password}")];
        let config = ContainerCreateBody {
            image: Some("redis:8-alpine".to_string()),
            cmd: Some(cmd),
            env: Some(env_vars.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    async fn create_mariadb_container(
        docker: &Docker, name: &str, network: &str, username: &str, password: &str,
        db_name: &str, port: u16, cpu_limit: f64, memory_limit_mb: i64, host_ip: Option<&str>,
    ) -> AppResult<String> {
        let env = [
            format!("MARIADB_USER={username}"),
            format!("MARIADB_PASSWORD={password}"),
            format!("MARIADB_DATABASE={db_name}"),
            format!("MARIADB_ROOT_PASSWORD={password}"),
            "MARIADB_AUTO_UPGRADE=1".to_string(),
        ];
        let host_config = Self::build_host_config(network, "3306/tcp", port, cpu_limit, memory_limit_mb, host_ip);
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Mariadb, password);
        let config = ContainerCreateBody {
            image: Some("mariadb:11-noble".to_string()),
            env: Some(env.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    // --- SNI container creators (no port bindings) ---

    async fn create_postgres_container_sni(
        docker: &Docker, name: &str, network: &str, username: &str, password: &str,
        db_name: &str, cpu_limit: f64, memory_limit_mb: i64,
    ) -> AppResult<String> {
        let env = [
            format!("POSTGRES_USER={username}"),
            format!("POSTGRES_PASSWORD={password}"),
            format!("POSTGRES_DB={db_name}"),
        ];
        let host_config = Self::build_host_config_sni(network, cpu_limit, memory_limit_mb);
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Postgresql, password);
        let config = ContainerCreateBody {
            image: Some("postgres:17-alpine".to_string()),
            env: Some(env.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    async fn create_redis_container_sni(
        docker: &Docker, name: &str, network: &str, password: &str,
        cpu_limit: f64, memory_limit_mb: i64,
    ) -> AppResult<String> {
        let host_config = Self::build_host_config_sni(network, cpu_limit, memory_limit_mb);
        let max_mem = format!("{memory_limit_mb}mb");
        let cmd: Vec<String> = vec![
            "redis-server".to_string(), "--requirepass".to_string(), password.to_string(),
            "--appendonly".to_string(), "yes".to_string(),
            "--maxmemory".to_string(), max_mem, "--maxmemory-policy".to_string(), "allkeys-lru".to_string(),
        ];
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Redis, password);
        let env_vars = [format!("REDIS_PASSWORD={password}")];
        let config = ContainerCreateBody {
            image: Some("redis:8-alpine".to_string()),
            cmd: Some(cmd),
            env: Some(env_vars.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    async fn create_mariadb_container_sni(
        docker: &Docker, name: &str, network: &str, username: &str, password: &str,
        db_name: &str, cpu_limit: f64, memory_limit_mb: i64,
    ) -> AppResult<String> {
        let env = [
            format!("MARIADB_USER={username}"),
            format!("MARIADB_PASSWORD={password}"),
            format!("MARIADB_DATABASE={db_name}"),
            format!("MARIADB_ROOT_PASSWORD={password}"),
            "MARIADB_AUTO_UPGRADE=1".to_string(),
        ];
        let host_config = Self::build_host_config_sni(network, cpu_limit, memory_limit_mb);
        let healthcheck = Self::healthcheck_for_db_type(&DbType::Mariadb, password);
        let config = ContainerCreateBody {
            image: Some("mariadb:11-noble".to_string()),
            env: Some(env.iter().map(|s| s.to_string()).collect()),
            host_config: Some(host_config),
            healthcheck: Some(healthcheck),
            ..Default::default()
        };
        let options = CreateContainerOptions { name: Some(name.to_string()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        Ok(response.id)
    }

    // --- Exec ---

    pub async fn exec_in_container(&self, docker: Option<&Docker>, container_id: &str, cmd: Vec<&str>) -> AppResult<String> {
        let docker = self.docker(docker);
        let exec = docker.create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd.into_iter().map(|s| s.to_string()).collect()),
                ..Default::default()
            },
        ).await?;

        let mut output = String::new();
        if let StartExecResults::Attached { output: mut exec_output, .. } =
            docker.start_exec(&exec.id, None).await?
        {
            while let Some(Ok(msg)) = exec_output.next().await {
                output.push_str(&format!("{msg}"));
            }
        }
        Ok(output)
    }

    // --- Backups ---

    pub async fn backup_postgres(
        &self, docker: Option<&Docker>, container_id: &str, username: &str, db_name: &str, backup_dir: &str, filename: &str,
    ) -> AppResult<u64> {
        let dump_path = format!("/tmp/{filename}");
        self.exec_in_container(
            docker, container_id,
            vec!["pg_dump", "-U", username, "-d", db_name, "-Fc", "-f", &dump_path],
        ).await?;

        self.copy_from_container(docker, container_id, &dump_path, backup_dir, filename).await
    }

    pub async fn backup_redis(
        &self, docker: Option<&Docker>, container_id: &str, password: &str, backup_dir: &str, filename: &str,
    ) -> AppResult<u64> {
        self.exec_in_container(
            docker, container_id,
            vec!["redis-cli", "-a", password, "--no-auth-warning", "BGSAVE"],
        ).await?;

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        self.copy_from_container(docker, container_id, "/data/dump.rdb", backup_dir, filename).await
    }

    pub async fn backup_mariadb(
        &self, docker: Option<&Docker>, container_id: &str, username: &str, password: &str, db_name: &str, backup_dir: &str, filename: &str,
    ) -> AppResult<u64> {
        let dump_path = format!("/tmp/{filename}");
        self.exec_in_container(
            docker, container_id,
            vec!["mariadb-dump", "-u", username, &format!("-p{password}"), "--single-transaction", "--routines", "--triggers", db_name, "--result-file", &dump_path],
        ).await?;

        self.copy_from_container(docker, container_id, &dump_path, backup_dir, filename).await
    }

    async fn copy_from_container(
        &self, target_docker: Option<&Docker>, container_id: &str, src_path: &str, backup_dir: &str, filename: &str,
    ) -> AppResult<u64> {
        let docker = self.docker(target_docker);
        let options = DownloadFromContainerOptions { path: src_path.to_string() };
        let mut stream = docker.download_from_container(container_id, Some(options));

        let dest = Path::new(backup_dir).join(filename);
        tokio::fs::create_dir_all(backup_dir).await
            .map_err(|e| AppError::Internal(format!("Failed to create backup dir: {e}")))?;

        let mut tar_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::Internal(format!("Download error: {e}")))?;
            tar_data.extend_from_slice(&chunk);
        }

        let dest_clone = dest.clone();
        let size = tokio::task::spawn_blocking(move || -> Result<u64, AppError> {
            let mut archive = tar::Archive::new(&tar_data[..]);
            let mut entries = archive.entries().map_err(|e| AppError::Internal(format!("Tar error: {e}")))?;
            let size = if let Some(entry) = entries.next() {
                let mut entry = entry.map_err(|e| AppError::Internal(format!("Tar entry error: {e}")))?;
                let sz = entry.size();
                let mut file_data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut file_data)
                    .map_err(|e| AppError::Internal(format!("Read error: {e}")))?;
                std::fs::write(&dest_clone, &file_data)
                    .map_err(|e| AppError::Internal(format!("Write error: {e}")))?;
                sz
            } else {
                0u64
            };
            Ok(size)
        }).await.map_err(|e| AppError::Internal(format!("Spawn error: {e}")))??;

        let _ = self.exec_in_container(target_docker, container_id, vec!["rm", "-f", src_path]).await;

        Ok(size)
    }

    // --- SQL escaping helpers ---

    /// Escape a value for use in single-quoted SQL strings (prevent SQL injection)
    fn escape_sql_literal(s: &str) -> String {
        s.replace('\'', "''")
    }

    /// Escape a value for use in double-quoted SQL identifiers
    fn escape_sql_identifier(s: &str) -> String {
        s.replace('"', "\"\"")
    }

    // --- User management: PostgreSQL ---

    pub async fn create_pg_user(
        &self, docker: Option<&Docker>, container_id: &str, owner_username: &str, db_name: &str,
        username: &str, password: &str, permission: &DbPermission,
    ) -> AppResult<()> {
        let esc_user = Self::escape_sql_identifier(username);
        let esc_pass = Self::escape_sql_literal(password);
        let create_role_sql = format!("CREATE ROLE \"{esc_user}\" WITH LOGIN PASSWORD '{esc_pass}';");
        self.exec_in_container(docker, container_id, vec!["psql", "-U", owner_username, "-d", db_name, "-c", &create_role_sql]).await?;

        let esc_db = Self::escape_sql_identifier(db_name);
        let grant_sql = match permission {
            DbPermission::Admin => format!(
                "GRANT ALL PRIVILEGES ON DATABASE \"{esc_db}\" TO \"{esc_user}\"; \
                 GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO \"{esc_user}\"; \
                 GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO \"{esc_user}\"; \
                 ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON TABLES TO \"{esc_user}\"; \
                 ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON SEQUENCES TO \"{esc_user}\";"
            ),
            DbPermission::ReadWrite => format!(
                "GRANT CONNECT ON DATABASE \"{esc_db}\" TO \"{esc_user}\"; \
                 GRANT USAGE ON SCHEMA public TO \"{esc_user}\"; \
                 GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO \"{esc_user}\"; \
                 GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO \"{esc_user}\"; \
                 ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO \"{esc_user}\"; \
                 ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO \"{esc_user}\";"
            ),
            DbPermission::ReadOnly => format!(
                "GRANT CONNECT ON DATABASE \"{esc_db}\" TO \"{esc_user}\"; \
                 GRANT USAGE ON SCHEMA public TO \"{esc_user}\"; \
                 GRANT SELECT ON ALL TABLES IN SCHEMA public TO \"{esc_user}\"; \
                 ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO \"{esc_user}\";"
            ),
        };
        self.exec_in_container(docker, container_id, vec!["psql", "-U", owner_username, "-d", db_name, "-c", &grant_sql]).await?;
        Ok(())
    }

    pub async fn remove_pg_user(&self, docker: Option<&Docker>, container_id: &str, owner_username: &str, db_name: &str, username: &str) -> AppResult<()> {
        let esc_user = Self::escape_sql_identifier(username);
        let esc_db = Self::escape_sql_identifier(db_name);
        let sql = format!(
            "REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM \"{esc_user}\"; \
             REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM \"{esc_user}\"; \
             REVOKE ALL PRIVILEGES ON DATABASE \"{esc_db}\" FROM \"{esc_user}\"; \
             REVOKE USAGE ON SCHEMA public FROM \"{esc_user}\"; \
             DROP ROLE IF EXISTS \"{esc_user}\";"
        );
        self.exec_in_container(docker, container_id, vec!["psql", "-U", owner_username, "-d", db_name, "-c", &sql]).await?;
        Ok(())
    }

    pub async fn rotate_pg_password(&self, docker: Option<&Docker>, container_id: &str, owner_username: &str, db_name: &str, username: &str, new_password: &str) -> AppResult<()> {
        let esc_user = Self::escape_sql_identifier(username);
        let esc_pass = Self::escape_sql_literal(new_password);
        let sql = format!("ALTER ROLE \"{esc_user}\" WITH PASSWORD '{esc_pass}';");
        self.exec_in_container(docker, container_id, vec!["psql", "-U", owner_username, "-d", db_name, "-c", &sql]).await?;
        Ok(())
    }

    // --- User management: Redis ---

    /// Validate a Redis username/password contains no spaces or control chars that could break ACL commands
    fn validate_redis_credential(val: &str, field: &str) -> AppResult<()> {
        if val.is_empty() || val.len() > 256 {
            return Err(AppError::BadRequest(format!("{field} has invalid length")));
        }
        if val.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(AppError::BadRequest(format!("{field} contains invalid characters")));
        }
        Ok(())
    }

    pub async fn create_redis_user(&self, docker: Option<&Docker>, container_id: &str, owner_password: &str, username: &str, password: &str, permission: &DbPermission) -> AppResult<()> {
        Self::validate_redis_credential(username, "Redis username")?;
        Self::validate_redis_credential(password, "Redis password")?;

        let pass_arg = format!(">{password}");
        // Pass ACL command as separate arguments to redis-cli, not as a single string
        let mut acl_args = vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "SETUSER", username, "on", &pass_arg, "~*", "&*"];
        let extra: Vec<&str> = match permission {
            DbPermission::Admin => vec!["+@all"],
            DbPermission::ReadWrite => vec!["+@read", "+@write", "+@connection", "+@string", "+@list", "+@set", "+@sortedset", "+@hash", "+@stream", "-@admin", "-@dangerous"],
            DbPermission::ReadOnly => vec!["+@read", "+@connection", "-@admin", "-@dangerous", "-@write"],
        };
        acl_args.extend(extra);
        self.exec_in_container(docker, container_id, acl_args).await?;
        self.exec_in_container(docker, container_id, vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "SAVE"]).await?;
        Ok(())
    }

    pub async fn remove_redis_user(&self, docker: Option<&Docker>, container_id: &str, owner_password: &str, username: &str) -> AppResult<()> {
        Self::validate_redis_credential(username, "Redis username")?;
        self.exec_in_container(docker, container_id, vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "DELUSER", username]).await?;
        self.exec_in_container(docker, container_id, vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "SAVE"]).await?;
        Ok(())
    }

    pub async fn rotate_redis_password(&self, docker: Option<&Docker>, container_id: &str, owner_password: &str, username: &str, new_password: &str) -> AppResult<()> {
        Self::validate_redis_credential(username, "Redis username")?;
        Self::validate_redis_credential(new_password, "Redis password")?;
        let pass_arg = format!(">{new_password}");
        self.exec_in_container(docker, container_id, vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "SETUSER", username, "resetpass", &pass_arg]).await?;
        self.exec_in_container(docker, container_id, vec!["redis-cli", "-a", owner_password, "--no-auth-warning", "ACL", "SAVE"]).await?;
        Ok(())
    }

    // --- User management: MariaDB ---

    pub async fn create_mariadb_user(&self, docker: Option<&Docker>, container_id: &str, root_password: &str, db_name: &str, username: &str, password: &str, permission: &DbPermission) -> AppResult<()> {
        let esc_user = Self::escape_sql_literal(username);
        let esc_pass = Self::escape_sql_literal(password);
        let esc_db = Self::escape_sql_literal(db_name);
        let create_sql = format!("CREATE USER '{esc_user}'@'%' IDENTIFIED BY '{esc_pass}';");
        self.exec_in_container(docker, container_id, vec!["mariadb", "-uroot", &format!("-p{root_password}"), "-e", &create_sql]).await?;

        let grant_sql = match permission {
            DbPermission::Admin => format!("GRANT ALL PRIVILEGES ON `{esc_db}`.* TO '{esc_user}'@'%' WITH GRANT OPTION;"),
            DbPermission::ReadWrite => format!("GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, ALTER, INDEX, DROP ON `{esc_db}`.* TO '{esc_user}'@'%';"),
            DbPermission::ReadOnly => format!("GRANT SELECT ON `{esc_db}`.* TO '{esc_user}'@'%';"),
        };
        self.exec_in_container(docker, container_id, vec!["mariadb", "-uroot", &format!("-p{root_password}"), "-e", &grant_sql]).await?;
        self.exec_in_container(docker, container_id, vec!["mariadb", "-uroot", &format!("-p{root_password}"), "-e", "FLUSH PRIVILEGES;"]).await?;
        Ok(())
    }

    pub async fn remove_mariadb_user(&self, docker: Option<&Docker>, container_id: &str, root_password: &str, username: &str) -> AppResult<()> {
        let esc_user = Self::escape_sql_literal(username);
        let sql = format!("DROP USER IF EXISTS '{esc_user}'@'%';");
        self.exec_in_container(docker, container_id, vec!["mariadb", "-uroot", &format!("-p{root_password}"), "-e", &sql]).await?;
        Ok(())
    }

    pub async fn rotate_mariadb_password(&self, docker: Option<&Docker>, container_id: &str, root_password: &str, username: &str, new_password: &str) -> AppResult<()> {
        let esc_user = Self::escape_sql_literal(username);
        let esc_pass = Self::escape_sql_literal(new_password);
        let sql = format!("ALTER USER '{esc_user}'@'%' IDENTIFIED BY '{esc_pass}'; FLUSH PRIVILEGES;");
        self.exec_in_container(docker, container_id, vec!["mariadb", "-uroot", &format!("-p{root_password}"), "-e", &sql]).await?;
        Ok(())
    }

    // --- Container lifecycle ---

    pub async fn restart_container(&self, docker: Option<&Docker>, container_id: &str) -> AppResult<()> {
        self.docker(docker)
            .restart_container(container_id, Some(RestartContainerOptions { t: Some(30), signal: None }))
            .await
            .map_err(AppError::Docker)
    }

    pub async fn stop_container(&self, docker: Option<&Docker>, container_id: &str) -> AppResult<()> {
        self.docker(docker)
            .stop_container(container_id, Some(StopContainerOptions { t: Some(30), signal: None }))
            .await
            .map_err(AppError::Docker)
    }

    pub async fn start_container(&self, docker: Option<&Docker>, container_id: &str) -> AppResult<()> {
        self.docker(docker).start_container(container_id, None::<StartContainerOptions>).await.map_err(AppError::Docker)
    }

    pub async fn remove_container(&self, docker: Option<&Docker>, container_id: &str, network_id: &str) -> AppResult<()> {
        let d = self.docker(docker);
        let _ = d.stop_container(container_id, Some(StopContainerOptions { t: Some(30), signal: None })).await;
        d.remove_container(container_id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await?;
        d.remove_network(network_id).await?;
        Ok(())
    }

    // --- Private Networks ---

    pub async fn create_private_network(&self, target: Option<&Docker>, name: &str) -> AppResult<(String, Option<String>, Option<String>)> {
        let docker = self.docker(target);
        let net_id = Self::create_network(docker, name).await?;

        // Inspect to get subnet/gateway
        let mut subnet = None;
        let mut gateway = None;
        if let Ok(info) = docker.inspect_network(&net_id, None).await {
            if let Some(ipam) = info.ipam {
                if let Some(configs) = ipam.config {
                    if let Some(first) = configs.first() {
                        subnet = first.subnet.clone();
                        gateway = first.gateway.clone();
                    }
                }
            }
        }

        Ok((net_id, subnet, gateway))
    }

    pub async fn attach_container_to_network(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        network_id: &str,
    ) -> AppResult<()> {
        let docker = self.docker(target);
        docker
            .connect_network(
                network_id,
                NetworkConnectRequest {
                    container: container_id.to_string(),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    pub async fn detach_container_from_network(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        network_id: &str,
    ) -> AppResult<()> {
        let docker = self.docker(target);
        docker
            .disconnect_network(
                network_id,
                NetworkDisconnectRequest {
                    container: container_id.to_string(),
                    force: Some(false),
                },
            )
            .await?;
        Ok(())
    }

    pub async fn remove_private_network(&self, target: Option<&Docker>, network_id: &str) -> AppResult<()> {
        let docker = self.docker(target);
        docker.remove_network(network_id).await?;
        Ok(())
    }

    // --- Network Peering ---

    /// Create a peering bridge network with ICC=false (inter-container communication disabled).
    /// Traffic between containers on this bridge is blocked by default — iptables rules open specific ports.
    pub async fn create_peering_network(&self, target: Option<&Docker>, name: &str) -> AppResult<String> {
        let docker = self.docker(target);
        let mut options_map = HashMap::new();
        options_map.insert("com.docker.network.bridge.enable_icc".to_string(), "false".to_string());

        let options = NetworkCreateRequest {
            name: name.to_string(),
            driver: Some("bridge".to_string()),
            internal: Some(false),
            options: Some(options_map),
            ..Default::default()
        };
        let response = docker.create_network(options).await?;
        let id = response.id;
        if id.is_empty() {
            return Err(AppError::Internal("Peering network creation returned no ID".to_string()));
        }
        Ok(id)
    }

    /// Execute a command on the Docker host via an ephemeral alpine container with --net=host --privileged.
    /// Used for iptables manipulation.
    /// Execute a script on the host network namespace via the dbsaas-nftables helper image.
    /// The image must be pre-built (see nftables-helper/Dockerfile).
    /// Uses --network host + --userns host + NET_ADMIN to manipulate host nftables.
    pub async fn exec_on_host(&self, target: Option<&Docker>, script: &str) -> AppResult<String> {
        let docker = self.docker(target);

        let container_name = format!("sb-nft-{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let config = ContainerCreateBody {
            image: Some("dbsaas-nftables:latest".to_string()),
            cmd: Some(vec!["sh".to_string(), "-c".to_string(), script.to_string()]),
            host_config: Some(HostConfig {
                network_mode: Some("host".to_string()),
                userns_mode: Some("host".to_string()),
                auto_remove: Some(true),
                cap_add: Some(vec!["NET_ADMIN".to_string(), "NET_RAW".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptions { name: Some(container_name.clone()), ..Default::default() };
        let response = docker.create_container(Some(options), config).await?;
        let cid = &response.id;

        docker.start_container(cid, None::<StartContainerOptions>).await?;

        // Collect output
        use bollard::query_parameters::LogsOptions;
        let mut output = String::new();
        let mut logs = docker.logs(
            cid,
            Some(LogsOptions { follow: true, stdout: true, stderr: true, ..Default::default() }),
        );
        while let Some(Ok(msg)) = logs.next().await {
            output.push_str(&format!("{msg}"));
        }

        Ok(output)
    }

    /// Validate a value contains only safe characters for nftables commands
    fn validate_nft_value(val: &str, field_name: &str) -> AppResult<()> {
        // Only allow alphanumeric, dots, slashes (for CIDR), hyphens, underscores
        if !val.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '/' || c == '-' || c == '_' || c == ':') {
            return Err(AppError::BadRequest(format!("Invalid characters in {field_name}: {val}")));
        }
        if val.is_empty() || val.len() > 64 {
            return Err(AppError::BadRequest(format!("{field_name} has invalid length")));
        }
        Ok(())
    }

    /// Apply nftables firewall rules for a peering bridge.
    /// Creates a table sb_peer_{id} with a forward chain filtering on the bridge interface.
    pub async fn apply_firewall_rules(
        &self,
        target: Option<&Docker>,
        peering_id: &str,
        bridge_name: &str,
        rules: &[FirewallRule],
    ) -> AppResult<()> {
        // Sanitize all inputs to prevent shell injection
        Self::validate_nft_value(peering_id, "peering_id")?;
        Self::validate_nft_value(bridge_name, "bridge_name")?;

        let table = format!("sb_peer_{}", &peering_id[..8.min(peering_id.len())]);
        let mut script = String::new();

        // Delete table if exists (atomic flush + recreate)
        script.push_str(&format!("nft delete table inet {table} 2>/dev/null; "));

        // Create table and chain
        script.push_str(&format!(
            "nft add table inet {table}; \
             nft add chain inet {table} forward {{ type filter hook forward priority -1\\; policy accept\\; }}; "
        ));

        // Allow established/related connections first
        script.push_str(&format!(
            "nft add rule inet {table} forward iifname {bridge_name} oifname {bridge_name} ct state established,related accept; "
        ));

        // Add each user rule (sorted by priority — caller should sort)
        for (src_subnet, dst_subnet, port, protocol, action) in rules {
            // Validate every field
            Self::validate_nft_value(src_subnet, "source subnet")?;
            Self::validate_nft_value(dst_subnet, "destination subnet")?;

            let verdict = if action == "allow" { "accept" } else { "drop" };
            let proto = protocol.as_deref().unwrap_or("tcp");
            Self::validate_nft_value(proto, "protocol")?;

            if let Some(p) = port {
                if *p < 1 || *p > 65535 {
                    return Err(AppError::BadRequest(format!("Invalid port number: {p}")));
                }
                script.push_str(&format!(
                    "nft add rule inet {table} forward iifname {bridge_name} oifname {bridge_name} \
                     ip saddr {src_subnet} ip daddr {dst_subnet} {proto} dport {p} {verdict}; "
                ));
            } else {
                script.push_str(&format!(
                    "nft add rule inet {table} forward iifname {bridge_name} oifname {bridge_name} \
                     ip saddr {src_subnet} ip daddr {dst_subnet} {verdict}; "
                ));
            }
        }

        // Default deny all other traffic on this bridge
        script.push_str(&format!(
            "nft add rule inet {table} forward iifname {bridge_name} oifname {bridge_name} drop; "
        ));

        script.push_str("echo OK");

        self.exec_on_host(target, &script).await?;
        Ok(())
    }

    /// Remove nftables table for a peering bridge.
    pub async fn remove_firewall_rules(
        &self,
        target: Option<&Docker>,
        peering_id: &str,
        _bridge_name: &str,
    ) -> AppResult<()> {
        let table = format!("sb_peer_{}", &peering_id[..8]);
        let script = format!(
            "nft delete table inet {table} 2>/dev/null; echo OK"
        );
        let _ = self.exec_on_host(target, &script).await;
        Ok(())
    }

    /// Check if a container is running
    pub async fn is_container_running(&self, target: Option<&Docker>, container_id: &str) -> AppResult<bool> {
        let docker = self.docker(target);
        match docker.inspect_container(container_id, None).await {
            Ok(info) => {
                let running = info
                    .state
                    .and_then(|s| s.running)
                    .unwrap_or(false);
                Ok(running)
            }
            Err(_) => Ok(false),
        }
    }

    /// Get one-shot container stats
    pub async fn get_container_stats_once(
        &self,
        target: Option<&Docker>,
        container_id: &str,
    ) -> AppResult<Option<ContainerStats>> {
        let docker = self.docker(target);
        let options = StatsOptions { stream: false, one_shot: true };
        let mut stream = docker.stats(container_id, Some(options));

        if let Some(Ok(stats)) = stream.next().await {
            let cpu_stats = stats.cpu_stats.unwrap_or_default();
            let precpu_stats = stats.precpu_stats.unwrap_or_default();
            let cpu_delta = cpu_stats.cpu_usage.unwrap_or_default().total_usage.unwrap_or(0) as f64
                - precpu_stats.cpu_usage.unwrap_or_default().total_usage.unwrap_or(0) as f64;
            let system_delta = cpu_stats.system_cpu_usage.unwrap_or(0) as f64
                - precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
            let num_cpus = cpu_stats.online_cpus.unwrap_or(1) as f64;
            let cpu_percent = if system_delta > 0.0 {
                (cpu_delta / system_delta) * num_cpus * 100.0
            } else {
                0.0
            };
            let mem_stats = stats.memory_stats.unwrap_or_default();
            let memory_usage = mem_stats.usage.unwrap_or(0);
            let memory_limit = mem_stats.limit.unwrap_or(0);
            let memory_percent = if memory_limit > 0 {
                (memory_usage as f64 / memory_limit as f64) * 100.0
            } else {
                0.0
            };
            Ok(Some(ContainerStats {
                cpu_percent,
                memory_usage,
                memory_limit,
                memory_percent,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get disk usage for a database container
    pub async fn get_disk_usage(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        db_type: &DbType,
    ) -> AppResult<u64> {
        let data_dir = match db_type {
            DbType::Postgresql => "/var/lib/postgresql/data",
            DbType::Redis => "/data",
            DbType::Mariadb => "/var/lib/mysql",
        };
        let output = self.exec_in_container(
            target,
            container_id,
            vec!["du", "-sb", data_dir],
        ).await?;
        let size: u64 = output
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Ok(size)
    }

    /// Update container resource limits (vertical scaling)
    pub async fn update_container_resources(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        cpu_limit: f64,
        memory_limit_mb: i32,
    ) -> AppResult<()> {
        let docker = self.docker(target);
        let nano_cpus = (cpu_limit * 1_000_000_000.0) as i64;
        let memory = (memory_limit_mb as i64) * 1024 * 1024;

        let update_config = ContainerUpdateBody {
            memory: Some(memory),
            memory_swap: Some(memory),
            nano_cpus: Some(nano_cpus),
            ..Default::default()
        };

        docker.update_container(container_id, update_config).await?;
        Ok(())
    }

    /// Restore a PostgreSQL backup into a container
    pub async fn restore_postgres(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        username: &str,
        _password: &str,
        db_name: &str,
        backup_path: &str,
    ) -> AppResult<()> {
        // Upload the backup file into the container
        self.upload_file_to_container(target, container_id, backup_path, "/tmp/restore.dump").await?;
        // Restore using pg_restore
        self.exec_in_container(
            target,
            container_id,
            vec!["pg_restore", "-U", username, "-d", db_name, "--clean", "--if-exists", "/tmp/restore.dump"],
        ).await?;
        Ok(())
    }

    /// Restore a MariaDB backup into a container
    pub async fn restore_mariadb(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        username: &str,
        password: &str,
        db_name: &str,
        backup_path: &str,
    ) -> AppResult<()> {
        self.upload_file_to_container(target, container_id, backup_path, "/tmp/restore.sql").await?;
        // Use mariadb CLI with --execute to source the file, avoiding shell injection via sh -c
        let password_arg = format!("-p{password}");
        let user_arg = format!("-u{username}");
        self.exec_in_container(
            target,
            container_id,
            vec!["mariadb", &user_arg, &password_arg, db_name, "-e", "source /tmp/restore.sql"],
        ).await?;
        Ok(())
    }

    /// Restore a Redis backup into a container
    pub async fn restore_redis(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        backup_path: &str,
    ) -> AppResult<()> {
        self.upload_file_to_container(target, container_id, backup_path, "/data/dump.rdb").await?;
        // Restart to load the dump
        self.restart_container(target, container_id).await?;
        Ok(())
    }

    /// Upload a file into a container
    async fn upload_file_to_container(
        &self,
        target: Option<&Docker>,
        container_id: &str,
        host_path: &str,
        container_path: &str,
    ) -> AppResult<()> {
        let docker = self.docker(target);
        let data = tokio::fs::read(host_path).await.map_err(|e| {
            AppError::Internal(format!("Failed to read file {host_path}: {e}"))
        })?;

        // Create a tar archive with the file
        let container_dir = std::path::Path::new(container_path)
            .parent()
            .unwrap_or(std::path::Path::new("/"));
        let filename = std::path::Path::new(container_path)
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("restore"))
            .to_string_lossy();

        let mut tar_buf = Vec::new();
        {
            let mut tar_builder = tar::Builder::new(&mut tar_buf);
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder.append_data(&mut header, &*filename, &data[..]).map_err(|e| {
                AppError::Internal(format!("Failed to create tar: {e}"))
            })?;
            tar_builder.finish().map_err(|e| {
                AppError::Internal(format!("Failed to finish tar: {e}"))
            })?;
        }

        docker.upload_to_container(
            container_id,
            Some(UploadToContainerOptions {
                path: container_dir.to_string_lossy().to_string(),
                ..Default::default()
            }),
            bollard::body_stream(futures_util::stream::once(async { bytes::Bytes::from(tar_buf) })),
        ).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ContainerStats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
}
