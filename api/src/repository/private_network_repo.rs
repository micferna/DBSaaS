use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{FirewallRule, NetworkPeering, PrivateNetwork, PrivateNetworkMember, PrivateNetworkMemberInfo};

pub struct PrivateNetworkRepository;

impl PrivateNetworkRepository {
    pub async fn create(pool: &PgPool, user_id: Uuid, name: &str) -> AppResult<PrivateNetwork> {
        let row = sqlx::query_as::<_, PrivateNetwork>(
            "INSERT INTO private_networks (user_id, name) VALUES ($1, $2) RETURNING *",
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<PrivateNetwork>> {
        let row = sqlx::query_as::<_, PrivateNetwork>(
            "SELECT * FROM private_networks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<PrivateNetwork>> {
        let rows = sqlx::query_as::<_, PrivateNetwork>(
            "SELECT * FROM private_networks WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<PrivateNetwork>> {
        let rows = sqlx::query_as::<_, PrivateNetwork>(
            "SELECT * FROM private_networks ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM private_networks WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM private_networks WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_docker_network_id(
        pool: &PgPool,
        id: Uuid,
        docker_network_id: &str,
    ) -> AppResult<()> {
        sqlx::query("UPDATE private_networks SET docker_network_id = $1 WHERE id = $2")
            .bind(docker_network_id)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_subnet_info(
        pool: &PgPool,
        id: Uuid,
        subnet: Option<&str>,
        gateway: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query("UPDATE private_networks SET subnet = $1, gateway = $2 WHERE id = $3")
            .bind(subnet)
            .bind(gateway)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_docker_server_id(
        pool: &PgPool,
        id: Uuid,
        server_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query("UPDATE private_networks SET docker_server_id = $1 WHERE id = $2")
            .bind(server_id)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn add_member(
        pool: &PgPool,
        network_id: Uuid,
        database_id: Uuid,
    ) -> AppResult<PrivateNetworkMember> {
        let row = sqlx::query_as::<_, PrivateNetworkMember>(
            "INSERT INTO private_network_members (network_id, database_id) VALUES ($1, $2) RETURNING *",
        )
        .bind(network_id)
        .bind(database_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn remove_member(
        pool: &PgPool,
        network_id: Uuid,
        database_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM private_network_members WHERE network_id = $1 AND database_id = $2",
        )
        .bind(network_id)
        .bind(database_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn count_members(pool: &PgPool, network_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM private_network_members WHERE network_id = $1",
        )
        .bind(network_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    pub async fn find_members_with_db_info(
        pool: &PgPool,
        network_id: Uuid,
    ) -> AppResult<Vec<PrivateNetworkMemberInfo>> {
        let rows = sqlx::query_as::<_, PrivateNetworkMemberInfo>(
            "SELECT m.database_id, d.name as database_name, d.db_type,
                    CONCAT('sb-', d.id) as hostname, d.port, m.joined_at
             FROM private_network_members m
             JOIN database_instances d ON d.id = m.database_id
             WHERE m.network_id = $1
             ORDER BY m.joined_at",
        )
        .bind(network_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_networks_for_database(
        pool: &PgPool,
        database_id: Uuid,
    ) -> AppResult<Vec<PrivateNetwork>> {
        let rows = sqlx::query_as::<_, PrivateNetwork>(
            "SELECT n.* FROM private_networks n
             JOIN private_network_members m ON m.network_id = n.id
             WHERE m.database_id = $1",
        )
        .bind(database_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn is_member(
        pool: &PgPool,
        network_id: Uuid,
        database_id: Uuid,
    ) -> AppResult<bool> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM private_network_members WHERE network_id = $1 AND database_id = $2",
        )
        .bind(network_id)
        .bind(database_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    // --- Network Peerings ---

    pub async fn create_peering(
        pool: &PgPool,
        user_id: Uuid,
        network_a_id: Uuid,
        network_b_id: Uuid,
        docker_server_id: Option<Uuid>,
    ) -> AppResult<NetworkPeering> {
        let row = sqlx::query_as::<_, NetworkPeering>(
            "INSERT INTO network_peerings (user_id, network_a_id, network_b_id, docker_server_id)
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(user_id)
        .bind(network_a_id)
        .bind(network_b_id)
        .bind(docker_server_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_peering_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<NetworkPeering>> {
        let row = sqlx::query_as::<_, NetworkPeering>(
            "SELECT * FROM network_peerings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_peerings_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<NetworkPeering>> {
        let rows = sqlx::query_as::<_, NetworkPeering>(
            "SELECT * FROM network_peerings WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_peerings_for_network(pool: &PgPool, network_id: Uuid) -> AppResult<Vec<NetworkPeering>> {
        let rows = sqlx::query_as::<_, NetworkPeering>(
            "SELECT * FROM network_peerings WHERE network_a_id = $1 OR network_b_id = $1",
        )
        .bind(network_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn peering_exists(pool: &PgPool, a: Uuid, b: Uuid) -> AppResult<bool> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM network_peerings
             WHERE LEAST(network_a_id, network_b_id) = LEAST($1, $2)
               AND GREATEST(network_a_id, network_b_id) = GREATEST($1, $2)",
        )
        .bind(a)
        .bind(b)
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn update_peering_status(pool: &PgPool, id: Uuid, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE network_peerings SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_peering_bridge(pool: &PgPool, id: Uuid, bridge_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE network_peerings SET docker_bridge_id = $1 WHERE id = $2")
            .bind(bridge_id)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_peering(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM network_peerings WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_peerings_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM network_peerings WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    pub async fn list_all_peerings(pool: &PgPool) -> AppResult<Vec<NetworkPeering>> {
        let rows = sqlx::query_as::<_, NetworkPeering>(
            "SELECT * FROM network_peerings ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    // --- Firewall Rules ---

    pub async fn create_firewall_rule(
        pool: &PgPool,
        peering_id: Uuid,
        priority: i32,
        action: &str,
        source_network_id: Uuid,
        dest_network_id: Uuid,
        port: Option<i32>,
        protocol: Option<&str>,
        description: Option<&str>,
    ) -> AppResult<FirewallRule> {
        let row = sqlx::query_as::<_, FirewallRule>(
            "INSERT INTO firewall_rules (peering_id, priority, action, source_network_id, dest_network_id, port, protocol, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
        )
        .bind(peering_id)
        .bind(priority)
        .bind(action)
        .bind(source_network_id)
        .bind(dest_network_id)
        .bind(port)
        .bind(protocol)
        .bind(description)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_rules_by_peering(pool: &PgPool, peering_id: Uuid) -> AppResult<Vec<FirewallRule>> {
        let rows = sqlx::query_as::<_, FirewallRule>(
            "SELECT * FROM firewall_rules WHERE peering_id = $1 ORDER BY priority, created_at",
        )
        .bind(peering_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_rule_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<FirewallRule>> {
        let row = sqlx::query_as::<_, FirewallRule>(
            "SELECT * FROM firewall_rules WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_firewall_rule(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM firewall_rules WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_rules_by_peering(pool: &PgPool, peering_id: Uuid) -> AppResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM firewall_rules WHERE peering_id = $1",
        )
        .bind(peering_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }
}
