use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{CreateDockerServerRequest, DockerServer, UpdateDockerServerRequest};

pub struct DockerServerRepository;

impl DockerServerRepository {
    pub async fn create(pool: &PgPool, req: &CreateDockerServerRequest) -> AppResult<DockerServer> {
        let server = sqlx::query_as::<_, DockerServer>(
            "INSERT INTO docker_servers (name, url, tls_ca, tls_cert, tls_key, max_containers, region, notes, server_type)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *"
        )
        .bind(&req.name)
        .bind(&req.url)
        .bind(&req.tls_ca)
        .bind(&req.tls_cert)
        .bind(&req.tls_key)
        .bind(req.max_containers.unwrap_or(50))
        .bind(&req.region)
        .bind(&req.notes)
        .bind(req.server_type.as_deref().unwrap_or("client"))
        .fetch_one(pool)
        .await?;

        Ok(server)
    }

    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<DockerServer>> {
        let servers = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await?;

        Ok(servers)
    }

    pub async fn list_active(pool: &PgPool) -> AppResult<Vec<DockerServer>> {
        let servers = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers WHERE active = true ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await?;

        Ok(servers)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DockerServer>> {
        let server = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(server)
    }

    pub async fn update(pool: &PgPool, id: Uuid, req: &UpdateDockerServerRequest) -> AppResult<DockerServer> {
        let current = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers WHERE id = $1"
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        let server = sqlx::query_as::<_, DockerServer>(
            "UPDATE docker_servers SET
                name = $2, url = $3, tls_ca = $4, tls_cert = $5, tls_key = $6,
                max_containers = $7, active = $8, region = $9, notes = $10, server_type = $11
             WHERE id = $1
             RETURNING *"
        )
        .bind(id)
        .bind(req.name.as_deref().unwrap_or(&current.name))
        .bind(req.url.as_deref().unwrap_or(&current.url))
        .bind(req.tls_ca.as_ref().or(current.tls_ca.as_ref()))
        .bind(req.tls_cert.as_ref().or(current.tls_cert.as_ref()))
        .bind(req.tls_key.as_ref().or(current.tls_key.as_ref()))
        .bind(req.max_containers.unwrap_or(current.max_containers))
        .bind(req.active.unwrap_or(current.active))
        .bind(req.region.as_ref().or(current.region.as_ref()))
        .bind(req.notes.as_ref().or(current.notes.as_ref()))
        .bind(req.server_type.as_deref().unwrap_or(&current.server_type))
        .fetch_one(pool)
        .await?;

        Ok(server)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM docker_servers WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_last_seen(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE docker_servers SET last_seen_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_containers_on_server(pool: &PgPool, server_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM database_instances WHERE docker_server_id = $1 AND status NOT IN ('error', 'deleting')"
        )
        .bind(server_id)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    pub async fn find_active_client_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DockerServer>> {
        let server = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers WHERE id = $1 AND active = true AND server_type = 'client'"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(server)
    }

    pub async fn list_active_client(pool: &PgPool) -> AppResult<Vec<DockerServer>> {
        let servers = sqlx::query_as::<_, DockerServer>(
            "SELECT * FROM docker_servers WHERE active = true AND server_type = 'client' ORDER BY name ASC"
        )
        .fetch_all(pool)
        .await?;

        Ok(servers)
    }

    /// Select the best client Docker server for container placement.
    /// Picks the active client server with the most available capacity (max_containers - current count).
    pub async fn select_best_client_server(pool: &PgPool) -> AppResult<Option<DockerServer>> {
        let server = sqlx::query_as::<_, DockerServer>(
            "SELECT ds.* FROM docker_servers ds
             LEFT JOIN (
                 SELECT docker_server_id, COUNT(*) as cnt
                 FROM database_instances
                 WHERE status NOT IN ('error', 'deleting')
                 GROUP BY docker_server_id
             ) di ON ds.id = di.docker_server_id
             WHERE ds.active = true AND ds.server_type = 'client'
               AND (COALESCE(di.cnt, 0) < ds.max_containers)
             ORDER BY (ds.max_containers - COALESCE(di.cnt, 0)) DESC
             LIMIT 1"
        )
        .fetch_optional(pool)
        .await?;

        Ok(server)
    }
}
