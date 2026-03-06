use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{BackupRecord, Bundle, DatabaseInstance, DatabaseUser, DbPermission, DbStatus, DbType};

pub struct DatabaseRepository;

const SELECT_COLS: &str = "id, user_id, name, db_type, status, container_id, network_id, host, port, username, password_encrypted, database_name, tls_cert, cpu_limit, memory_limit_mb, bundle_id, tls_mode, plan_template_id, subdomain, routing_mode, docker_server_id, created_at, updated_at";

impl DatabaseRepository {
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        name: &str,
        db_type: &DbType,
        host: &str,
        port: i32,
        username: &str,
        password_encrypted: &str,
        database_name: Option<&str>,
        cpu_limit: f64,
        memory_limit_mb: i32,
        bundle_id: Option<Uuid>,
        tls_mode: &str,
        plan_template_id: Option<Uuid>,
        docker_server_id: Option<Uuid>,
        subdomain: &str,
        routing_mode: &str,
    ) -> AppResult<DatabaseInstance> {
        let q = format!(
            "INSERT INTO database_instances (user_id, name, db_type, host, port, username, password_encrypted, database_name, cpu_limit, memory_limit_mb, bundle_id, tls_mode, plan_template_id, docker_server_id, subdomain, routing_mode) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) \
             RETURNING {SELECT_COLS}"
        );
        let db = sqlx::query_as::<_, DatabaseInstance>(&q)
            .bind(user_id)
            .bind(name)
            .bind(db_type)
            .bind(host)
            .bind(port)
            .bind(username)
            .bind(password_encrypted)
            .bind(database_name)
            .bind(cpu_limit)
            .bind(memory_limit_mb)
            .bind(bundle_id)
            .bind(tls_mode)
            .bind(plan_template_id)
            .bind(docker_server_id)
            .bind(subdomain)
            .bind(routing_mode)
            .fetch_one(pool)
            .await?;
        Ok(db)
    }

    pub async fn update_provisioned(
        pool: &PgPool,
        id: Uuid,
        container_id: &str,
        network_id: &str,
        tls_cert: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE database_instances SET container_id = $1, network_id = $2, tls_cert = $3, status = 'running', updated_at = NOW() WHERE id = $4",
        )
        .bind(container_id)
        .bind(network_id)
        .bind(tls_cert)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_status(pool: &PgPool, id: Uuid, status: &DbStatus) -> AppResult<()> {
        sqlx::query("UPDATE database_instances SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_owner_password(
        pool: &PgPool,
        id: Uuid,
        password_encrypted: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE database_instances SET password_encrypted = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(password_encrypted)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DatabaseInstance>> {
        let q = format!("SELECT {SELECT_COLS} FROM database_instances WHERE id = $1");
        let db = sqlx::query_as::<_, DatabaseInstance>(&q)
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(db)
    }

    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<DatabaseInstance>> {
        let q = format!("SELECT {SELECT_COLS} FROM database_instances WHERE user_id = $1 ORDER BY created_at DESC");
        let dbs = sqlx::query_as::<_, DatabaseInstance>(&q)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(dbs)
    }

    pub async fn count_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM database_instances WHERE user_id = $1 AND status != 'deleting'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<DatabaseInstance>> {
        let q = format!("SELECT {SELECT_COLS} FROM database_instances ORDER BY created_at DESC");
        let dbs = sqlx::query_as::<_, DatabaseInstance>(&q)
            .fetch_all(pool)
            .await?;
        Ok(dbs)
    }

    pub async fn get_allocated_ports(pool: &PgPool) -> AppResult<Vec<i32>> {
        let ports: Vec<(i32,)> =
            sqlx::query_as("SELECT port FROM database_instances WHERE status != 'deleting'")
                .fetch_all(pool)
                .await?;
        Ok(ports.into_iter().map(|(p,)| p).collect())
    }

    pub async fn find_by_subdomain(pool: &PgPool, subdomain: &str) -> AppResult<Option<DatabaseInstance>> {
        let q = format!("SELECT {SELECT_COLS} FROM database_instances WHERE subdomain = $1");
        let db = sqlx::query_as::<_, DatabaseInstance>(&q)
            .bind(subdomain)
            .fetch_optional(pool)
            .await?;
        Ok(db)
    }

    pub async fn update_routing_mode(pool: &PgPool, id: Uuid, routing_mode: &str) -> AppResult<()> {
        sqlx::query("UPDATE database_instances SET routing_mode = $1, updated_at = NOW() WHERE id = $2")
            .bind(routing_mode)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM database_instances WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM database_instances")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    pub async fn find_databases_by_bundle(
        pool: &PgPool,
        bundle_id: Uuid,
    ) -> AppResult<Vec<DatabaseInstance>> {
        let q = format!("SELECT {SELECT_COLS} FROM database_instances WHERE bundle_id = $1 ORDER BY db_type");
        let dbs = sqlx::query_as::<_, DatabaseInstance>(&q)
            .bind(bundle_id)
            .fetch_all(pool)
            .await?;
        Ok(dbs)
    }

    // --- Bundle CRUD ---

    pub async fn create_bundle(pool: &PgPool, user_id: Uuid, name: &str) -> AppResult<Bundle> {
        let bundle = sqlx::query_as::<_, Bundle>(
            "INSERT INTO bundles (user_id, name) VALUES ($1, $2) RETURNING id, user_id, name, network_id, created_at",
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(pool)
        .await?;
        Ok(bundle)
    }

    pub async fn update_bundle_network(pool: &PgPool, id: Uuid, network_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE bundles SET network_id = $1 WHERE id = $2")
            .bind(network_id)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn find_bundle_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Bundle>> {
        let bundle = sqlx::query_as::<_, Bundle>(
            "SELECT id, user_id, name, network_id, created_at FROM bundles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(bundle)
    }

    pub async fn find_bundles_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Bundle>> {
        let bundles = sqlx::query_as::<_, Bundle>(
            "SELECT id, user_id, name, network_id, created_at FROM bundles WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(bundles)
    }

    pub async fn delete_bundle(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM bundles WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // --- Database Users CRUD ---

    pub async fn create_database_user(
        pool: &PgPool,
        database_id: Uuid,
        username: &str,
        password_encrypted: &str,
        permission: &DbPermission,
    ) -> AppResult<DatabaseUser> {
        let user = sqlx::query_as::<_, DatabaseUser>(
            r#"INSERT INTO database_users (database_id, username, password_encrypted, permission)
               VALUES ($1, $2, $3, $4)
               RETURNING id, database_id, username, password_encrypted, permission, created_at, updated_at"#,
        )
        .bind(database_id)
        .bind(username)
        .bind(password_encrypted)
        .bind(permission)
        .fetch_one(pool)
        .await?;
        Ok(user)
    }

    pub async fn find_database_users(pool: &PgPool, database_id: Uuid) -> AppResult<Vec<DatabaseUser>> {
        let users = sqlx::query_as::<_, DatabaseUser>(
            "SELECT id, database_id, username, password_encrypted, permission, created_at, updated_at FROM database_users WHERE database_id = $1 ORDER BY created_at",
        )
        .bind(database_id)
        .fetch_all(pool)
        .await?;
        Ok(users)
    }

    pub async fn find_database_user_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DatabaseUser>> {
        let user = sqlx::query_as::<_, DatabaseUser>(
            "SELECT id, database_id, username, password_encrypted, permission, created_at, updated_at FROM database_users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    pub async fn update_database_user_password(pool: &PgPool, id: Uuid, password_encrypted: &str) -> AppResult<()> {
        sqlx::query("UPDATE database_users SET password_encrypted = $1, updated_at = NOW() WHERE id = $2")
            .bind(password_encrypted)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_database_user(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM database_users WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_database_users(pool: &PgPool, database_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM database_users WHERE database_id = $1",
        )
        .bind(database_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    // --- Backups CRUD ---

    pub async fn create_backup(
        pool: &PgPool,
        database_id: Uuid,
        filename: &str,
        size_bytes: i64,
    ) -> AppResult<BackupRecord> {
        let backup = sqlx::query_as::<_, BackupRecord>(
            "INSERT INTO database_backups (database_id, filename, size_bytes) VALUES ($1, $2, $3) RETURNING id, database_id, filename, size_bytes, created_at",
        )
        .bind(database_id)
        .bind(filename)
        .bind(size_bytes)
        .fetch_one(pool)
        .await?;
        Ok(backup)
    }

    pub async fn find_backups_by_database(pool: &PgPool, database_id: Uuid) -> AppResult<Vec<BackupRecord>> {
        let backups = sqlx::query_as::<_, BackupRecord>(
            "SELECT id, database_id, filename, size_bytes, created_at FROM database_backups WHERE database_id = $1 ORDER BY created_at DESC",
        )
        .bind(database_id)
        .fetch_all(pool)
        .await?;
        Ok(backups)
    }

    pub async fn find_backup_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<BackupRecord>> {
        let backup = sqlx::query_as::<_, BackupRecord>(
            "SELECT id, database_id, filename, size_bytes, created_at FROM database_backups WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(backup)
    }

    pub async fn delete_backup(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM database_backups WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
