use axum::extract::{Path, State};
use crate::extract::Json;
use bollard::query_parameters::{ListContainersOptions, StatsOptions};
use futures_util::StreamExt;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    CreateDockerServerRequest, DockerServer, DockerServerStatus, UpdateDockerServerRequest,
};
use crate::repository::DockerServerRepository;
use crate::services::provisioner::ProvisionerService;
use crate::AppState;

/// GET /api/admin/servers — list all docker servers
pub async fn list_servers(State(state): State<AppState>) -> AppResult<Json<Vec<DockerServer>>> {
    let servers = DockerServerRepository::list_all(&state.db).await?;
    Ok(Json(servers))
}

/// POST /api/admin/servers — add a new docker server
pub async fn create_server(
    State(state): State<AppState>,
    Json(req): Json<CreateDockerServerRequest>,
) -> AppResult<Json<DockerServer>> {
    let server = DockerServerRepository::create(&state.db, &req).await?;
    Ok(Json(server))
}

/// PUT /api/admin/servers/{id} — update a docker server
pub async fn update_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDockerServerRequest>,
) -> AppResult<Json<DockerServer>> {
    let server = DockerServerRepository::update(&state.db, id, &req).await?;
    Ok(Json(server))
}

/// DELETE /api/admin/servers/{id} — remove a docker server
pub async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Check no containers assigned
    let count = DockerServerRepository::count_containers_on_server(&state.db, id).await?;
    if count > 0 {
        return Err(AppError::BadRequest(format!(
            "Server still has {count} active containers. Migrate them first."
        )));
    }
    DockerServerRepository::delete(&state.db, id).await?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

/// GET /api/admin/servers/status — live status of all docker servers
pub async fn servers_status(State(state): State<AppState>) -> AppResult<Json<Vec<DockerServerStatus>>> {
    let servers = DockerServerRepository::list_all(&state.db).await?;
    let mut statuses = Vec::with_capacity(servers.len());

    for server in servers {
        let status = check_server_health(&state, &server).await;
        statuses.push(status);
    }

    Ok(Json(statuses))
}

/// GET /api/admin/servers/{id}/status — live status of one docker server
pub async fn server_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DockerServerStatus>> {
    let server = DockerServerRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Server not found".to_string()))?;

    let status = check_server_health(&state, &server).await;
    Ok(Json(status))
}

/// GET /api/admin/servers/{id}/containers — list all containers on a server with stats
pub async fn server_containers(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let server = DockerServerRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Server not found".to_string()))?;

    let docker = ProvisionerService::connect_to_server(&server)?;

    let mut filters = std::collections::HashMap::new();
    filters.insert("status".to_string(), vec!["running".to_string()]);
    let options = ListContainersOptions {
        all: true,
        filters: Some(filters),
        ..Default::default()
    };

    let containers = docker.list_containers(Some(options)).await
        .map_err(|e| AppError::Internal(format!("Failed to list containers: {e}")))?;

    let mut result = Vec::new();
    for c in &containers {
        let container_id = c.id.as_deref().unwrap_or_default();
        let name = c.names.as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default();

        // Get one-shot stats
        let (cpu_pct, mem_usage, mem_limit) = get_container_stats(&docker, container_id).await;

        let state_str = c.state.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "unknown".to_string());
        let image = c.image.as_deref().unwrap_or("unknown");

        result.push(serde_json::json!({
            "id": container_id,
            "name": name,
            "image": image,
            "state": state_str,
            "cpu_percent": cpu_pct,
            "memory_usage_bytes": mem_usage,
            "memory_limit_bytes": mem_limit,
            "is_dbaas": name.starts_with("sb-"),
        }));
    }

    Ok(Json(result))
}

/// GET /api/admin/servers/{id}/resources — host resource usage (CPU, RAM, disk)
pub async fn server_resources(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let server = DockerServerRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Server not found".to_string()))?;

    let docker = ProvisionerService::connect_to_server(&server)?;

    let info = docker.info().await
        .map_err(|e| AppError::Internal(format!("Failed to get server info: {e}")))?;

    let df = docker.df(None::<bollard::query_parameters::DataUsageOptions>).await.ok();

    let images_count = df.as_ref().and_then(|d| d.images_disk_usage.as_ref()).and_then(|i| i.total_count).unwrap_or(0) as usize;
    let volumes_count = df.as_ref().and_then(|d| d.volumes_disk_usage.as_ref()).and_then(|v| v.total_count).unwrap_or(0) as usize;

    // Disk usage from Docker system df
    let images_size: i64 = df.as_ref()
        .and_then(|d| d.images_disk_usage.as_ref())
        .and_then(|i| i.total_size)
        .unwrap_or(0);
    let containers_size: i64 = df.as_ref()
        .and_then(|d| d.containers_disk_usage.as_ref())
        .and_then(|c| c.total_size)
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "cpu_count": info.ncpu,
        "memory_total_bytes": info.mem_total,
        "containers_running": info.containers_running,
        "containers_stopped": info.containers_stopped,
        "containers_total": info.containers,
        "images_count": images_count,
        "volumes_count": volumes_count,
        "images_size_bytes": images_size,
        "containers_size_bytes": containers_size,
        "os": info.operating_system,
        "kernel": info.kernel_version,
        "docker_root_dir": info.docker_root_dir,
    })))
}

/// Get one-shot CPU% and memory stats for a container
async fn get_container_stats(docker: &bollard::Docker, container_id: &str) -> (f64, i64, i64) {
    let options = StatsOptions { stream: false, one_shot: true };
    let mut stream = docker.stats(container_id, Some(options));

    if let Some(Ok(stats)) = stream.next().await {
        // CPU percent calculation
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
        let mem_usage = mem_stats.usage.unwrap_or(0) as i64;
        let mem_limit = mem_stats.limit.unwrap_or(0) as i64;

        (cpu_pct, mem_usage, mem_limit)
    } else {
        (0.0, 0, 0)
    }
}

/// Check a single docker server's health by connecting to it
async fn check_server_health(state: &AppState, server: &DockerServer) -> DockerServerStatus {
    let mut status = DockerServerStatus {
        id: server.id,
        name: server.name.clone(),
        url: server.url.clone(),
        region: server.region.clone(),
        active: server.active,
        server_type: server.server_type.clone(),
        max_containers: server.max_containers,
        online: false,
        containers_running: None,
        containers_total: None,
        cpu_count: None,
        memory_bytes: None,
        docker_version: None,
        last_seen_at: server.last_seen_at,
        error: None,
    };

    // Try connecting to the docker daemon
    let docker = match ProvisionerService::connect_to_server(server) {
        Ok(d) => d,
        Err(e) => {
            status.error = Some(format!("Connection failed: {e}"));
            return status;
        }
    };

    // Ping
    match docker.ping().await {
        Ok(_) => {
            status.online = true;
        }
        Err(e) => {
            status.error = Some(format!("Ping failed: {e}"));
            return status;
        }
    }

    // Get system info
    if let Ok(info) = docker.info().await {
        status.containers_running = info.containers_running;
        status.containers_total = info.containers;
        status.cpu_count = info.ncpu;
        status.memory_bytes = info.mem_total;
    }

    // Get version
    if let Ok(version) = docker.version().await {
        status.docker_version = version.version;
    }

    // Update last_seen
    let _ = DockerServerRepository::update_last_seen(&state.db, server.id).await;
    status.last_seen_at = Some(chrono::Utc::now());

    status
}
