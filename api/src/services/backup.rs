use sqlx::PgPool;
use std::sync::Arc;

use crate::repository::{BackupScheduleRepository, DatabaseRepository, DockerServerRepository};
use crate::services::provisioner::ProvisionerService;

pub struct BackupService;

impl BackupService {
    pub async fn run_scheduled_backups(
        pool: &PgPool,
        provisioner: &Arc<ProvisionerService>,
        backup_dir: &str,
        encryption_key: &str,
    ) {
        let due = match BackupScheduleRepository::find_due_schedules(pool).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch due backup schedules: {e}");
                return;
            }
        };

        for schedule in due {
            let db_inst = match DatabaseRepository::find_by_id(pool, schedule.database_id).await {
                Ok(Some(d)) => d,
                _ => continue,
            };

            let container_id = match db_inst.container_id.as_deref() {
                Some(cid) => cid,
                None => continue,
            };

            let target_docker = if let Some(server_id) = db_inst.docker_server_id {
                DockerServerRepository::find_by_id(pool, server_id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|s| ProvisionerService::connect_to_server(&s).ok())
            } else {
                None
            };

            let password = match crate::utils::crypto::decrypt_string(
                &db_inst.password_encrypted,
                encryption_key,
            ) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("{}_{}.sql", db_inst.name, timestamp);

            let result = match db_inst.db_type {
                crate::models::DbType::Postgresql => {
                    provisioner
                        .backup_postgres(target_docker.as_ref(), container_id, &db_inst.username, db_inst.database_name.as_deref().unwrap_or(&db_inst.name), backup_dir, &filename)
                        .await
                }
                crate::models::DbType::Mariadb => {
                    provisioner
                        .backup_mariadb(target_docker.as_ref(), container_id, &db_inst.username, &password, db_inst.database_name.as_deref().unwrap_or(&db_inst.name), backup_dir, &filename)
                        .await
                }
                crate::models::DbType::Redis => {
                    provisioner
                        .backup_redis(target_docker.as_ref(), container_id, &password, backup_dir, &filename)
                        .await
                }
            };

            match result {
                Ok(size_bytes) => {
                    if let Err(e) = DatabaseRepository::create_backup(pool, db_inst.id, &filename, size_bytes as i64).await {
                        tracing::error!("Failed to record backup for {}: {e}", db_inst.name);
                    }
                    if let Err(e) = BackupScheduleRepository::update_last_run(pool, schedule.id).await {
                        tracing::error!("Failed to update last_run for schedule {}: {e}", schedule.id);
                    }
                    tracing::info!("Scheduled backup created for {}: {filename}", db_inst.name);

                    // Cleanup old backups beyond retention count
                    Self::cleanup_old_backups(pool, db_inst.id, schedule.retention_count, backup_dir).await;
                }
                Err(e) => {
                    tracing::error!("Scheduled backup failed for {}: {e}", db_inst.name);
                }
            }
        }
    }

    async fn cleanup_old_backups(pool: &PgPool, database_id: uuid::Uuid, retention_count: i32, backup_dir: &str) {
        let backups = match DatabaseRepository::find_backups_by_database(pool, database_id).await {
            Ok(b) => b,
            Err(_) => return,
        };

        if backups.len() as i32 <= retention_count {
            return;
        }

        // Backups are ordered DESC by created_at, so skip the newest retention_count
        for backup in backups.iter().skip(retention_count as usize) {
            let path = format!("{}/{}", backup_dir, backup.filename);
            let _ = tokio::fs::remove_file(&path).await;
            let _ = DatabaseRepository::delete_backup(pool, backup.id).await;
            tracing::debug!("Cleaned up old backup: {}", backup.filename);
        }
    }
}
