use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::DbType;

// --- Network Peering ---

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct NetworkPeering {
    pub id: Uuid,
    pub user_id: Uuid,
    pub network_a_id: Uuid,
    pub network_b_id: Uuid,
    pub docker_bridge_id: Option<String>,
    pub docker_server_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FirewallRule {
    pub id: Uuid,
    pub peering_id: Uuid,
    pub priority: i32,
    pub action: String,
    pub source_network_id: Uuid,
    pub dest_network_id: Uuid,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePeeringRequest {
    pub network_a_id: Uuid,
    pub network_b_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateFirewallRuleRequest {
    pub action: String,
    pub source_network_id: Uuid,
    pub dest_network_id: Uuid,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub priority: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PeeringNetworkInfo {
    pub id: Uuid,
    pub name: String,
    pub member_count: i64,
}

#[derive(Debug, Serialize)]
pub struct PeeringResponse {
    pub id: Uuid,
    pub network_a: PeeringNetworkInfo,
    pub network_b: PeeringNetworkInfo,
    pub status: String,
    pub rules: Vec<FirewallRule>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PrivateNetwork {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub docker_network_id: Option<String>,
    pub docker_server_id: Option<Uuid>,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PrivateNetworkMember {
    pub id: Uuid,
    pub network_id: Uuid,
    pub database_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePrivateNetworkRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AttachDatabaseRequest {
    pub database_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct PrivateNetworkResponse {
    pub id: Uuid,
    pub name: String,
    pub docker_server_id: Option<Uuid>,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub members: Vec<PrivateNetworkMemberInfo>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PrivateNetworkMemberInfo {
    pub database_id: Uuid,
    pub database_name: String,
    pub db_type: DbType,
    pub hostname: String,
    pub port: i32,
    pub joined_at: DateTime<Utc>,
}
