use sqlx::PgPool;

use crate::services::provisioner::ProvisionerService;

pub async fn cleanup_stale_containers(pool: &PgPool, provisioner: &ProvisionerService) {
    let stale = sqlx::query_as::<_, (String, String)>(
        "SELECT container_id, network_id FROM database_instances WHERE status = 'deleting' AND container_id IS NOT NULL AND network_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await;

    if let Ok(rows) = stale {
        for (container_id, network_id) in rows {
            if let Err(e) = provisioner.remove_container(None, &container_id, &network_id).await {
                tracing::error!("Cleanup failed for container {container_id}: {e}");
            }
        }
    }
}
