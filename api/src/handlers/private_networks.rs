use axum::{
    extract::{Path, State},
    Extension,
};
use crate::extract::Json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthUser;
use crate::models::{DbStatus, PeeringNetworkInfo, PeeringResponse, PrivateNetworkResponse};
use crate::repository::{DatabaseRepository, DockerServerRepository, PrivateNetworkRepository};
use crate::services::provisioner::ProvisionerService;
use crate::AppState;

const MAX_NETWORKS_PER_USER: i64 = 10;
const MAX_MEMBERS_PER_NETWORK: i64 = 20;
const MAX_PEERINGS_PER_USER: i64 = 10;
const MAX_RULES_PER_PEERING: i64 = 20;

/// Resolve Docker client for a given server ID
async fn resolve_docker_for_server(
    state: &AppState,
    server_id: Option<Uuid>,
) -> AppResult<Option<bollard::Docker>> {
    if let Some(sid) = server_id {
        let server = DockerServerRepository::find_by_id(&state.db, sid)
            .await?
            .ok_or_else(|| AppError::Internal("Docker server not found".to_string()))?;
        let docker = ProvisionerService::connect_to_server(&server)?;
        Ok(Some(docker))
    } else {
        Ok(None)
    }
}

// POST /api/networks
pub async fn create_network(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<crate::models::CreatePrivateNetworkRequest>,
) -> AppResult<Json<PrivateNetworkResponse>> {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 63 {
        return Err(AppError::BadRequest("Name must be 1-63 characters".to_string()));
    }

    // Check limit
    let count = PrivateNetworkRepository::count_by_user(&state.db, user.id).await?;
    if count >= MAX_NETWORKS_PER_USER {
        return Err(AppError::BadRequest(format!(
            "Maximum {MAX_NETWORKS_PER_USER} networks per user"
        )));
    }

    let network = PrivateNetworkRepository::create(&state.db, user.id, &name).await?;

    Ok(Json(PrivateNetworkResponse {
        id: network.id,
        name: network.name,
        docker_server_id: network.docker_server_id,
        subnet: network.subnet,
        gateway: network.gateway,
        members: vec![],
        created_at: network.created_at,
    }))
}

// GET /api/networks
pub async fn list_networks(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<PrivateNetworkResponse>>> {
    let networks = PrivateNetworkRepository::find_by_user(&state.db, user.id).await?;
    let mut result = Vec::with_capacity(networks.len());

    for net in networks {
        let members =
            PrivateNetworkRepository::find_members_with_db_info(&state.db, net.id).await?;
        result.push(PrivateNetworkResponse {
            id: net.id,
            name: net.name,
            docker_server_id: net.docker_server_id,
            subnet: net.subnet,
            gateway: net.gateway,
            members,
            created_at: net.created_at,
        });
    }

    Ok(Json(result))
}

// GET /api/networks/{id}
pub async fn get_network(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PrivateNetworkResponse>> {
    let network = PrivateNetworkRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".to_string()))?;

    if network.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let members =
        PrivateNetworkRepository::find_members_with_db_info(&state.db, network.id).await?;

    Ok(Json(PrivateNetworkResponse {
        id: network.id,
        name: network.name,
        docker_server_id: network.docker_server_id,
        subnet: network.subnet,
        gateway: network.gateway,
        members,
        created_at: network.created_at,
    }))
}

// DELETE /api/networks/{id}
pub async fn delete_network(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let network = PrivateNetworkRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".to_string()))?;

    if network.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Clean up peerings associated with this network
    let peerings = PrivateNetworkRepository::find_peerings_for_network(&state.db, id).await?;
    for peering in &peerings {
        if let Some(ref bridge_id) = peering.docker_bridge_id {
            let target_docker = resolve_docker_for_server(&state, peering.docker_server_id).await?;
            let bridge_name = format!("sb-peer-{}", peering.id);

            // Remove iptables rules
            let _ = state.provisioner.remove_firewall_rules(
                target_docker.as_ref(),
                &peering.id.to_string(),
                &bridge_name,
            ).await;

            // Detach all containers from peering bridge
            let other_net_id = if peering.network_a_id == id { peering.network_b_id } else { peering.network_a_id };
            for net_id in [id, other_net_id] {
                let members = PrivateNetworkRepository::find_members_with_db_info(&state.db, net_id).await?;
                for member in &members {
                    if let Ok(Some(db)) = DatabaseRepository::find_by_id(&state.db, member.database_id).await {
                        if let Some(ref cid) = db.container_id {
                            let _ = state.provisioner.detach_container_from_network(target_docker.as_ref(), cid, bridge_id).await;
                        }
                    }
                }
            }

            // Remove peering bridge
            let _ = state.provisioner.remove_private_network(target_docker.as_ref(), bridge_id).await;
        }
        PrivateNetworkRepository::delete_peering(&state.db, peering.id).await?;
    }

    // Detach all members from the Docker network
    if let Some(ref docker_net_id) = network.docker_network_id {
        let target_docker = resolve_docker_for_server(&state, network.docker_server_id).await?;
        let members =
            PrivateNetworkRepository::find_members_with_db_info(&state.db, network.id).await?;

        for member in &members {
            // Get container_id for this database
            if let Ok(Some(db)) =
                DatabaseRepository::find_by_id(&state.db, member.database_id).await
            {
                if let Some(ref container_id) = db.container_id {
                    let _ = state
                        .provisioner
                        .detach_container_from_network(
                            target_docker.as_ref(),
                            container_id,
                            docker_net_id,
                        )
                        .await;
                }
            }
        }

        // Remove the Docker network
        let _ = state
            .provisioner
            .remove_private_network(target_docker.as_ref(), docker_net_id)
            .await;
    }

    // Delete from DB (CASCADE removes members)
    PrivateNetworkRepository::delete(&state.db, id).await?;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// POST /api/networks/{id}/attach
pub async fn attach_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(network_id): Path<Uuid>,
    Json(req): Json<crate::models::AttachDatabaseRequest>,
) -> AppResult<Json<PrivateNetworkResponse>> {
    // 1. Network belongs to user
    let mut network = PrivateNetworkRepository::find_by_id(&state.db, network_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".to_string()))?;

    if network.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // 2. DB belongs to user
    let db = DatabaseRepository::find_by_id(&state.db, req.database_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database not found".to_string()))?;

    if db.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // 3. DB must be running with a container_id
    if db.status != DbStatus::Running {
        return Err(AppError::BadRequest(
            "Database must be running to attach".to_string(),
        ));
    }

    let container_id = db
        .container_id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Database has no container".to_string()))?;

    // 4. Same Docker server constraint
    if let Some(net_server_id) = network.docker_server_id {
        if db.docker_server_id != Some(net_server_id) {
            return Err(AppError::BadRequest(
                "Database must be on the same Docker server as the network".to_string(),
            ));
        }
    }

    // 5. Not already a member
    if PrivateNetworkRepository::is_member(&state.db, network_id, req.database_id).await? {
        return Err(AppError::Conflict(
            "Database is already a member of this network".to_string(),
        ));
    }

    // Check member limit
    let member_count = PrivateNetworkRepository::count_members(&state.db, network_id).await?;
    if member_count >= MAX_MEMBERS_PER_NETWORK {
        return Err(AppError::BadRequest(format!(
            "Maximum {MAX_MEMBERS_PER_NETWORK} members per network"
        )));
    }

    // If network has no server yet, set it from the first DB
    if network.docker_server_id.is_none() {
        if let Some(server_id) = db.docker_server_id {
            PrivateNetworkRepository::update_docker_server_id(&state.db, network_id, server_id)
                .await?;
            network.docker_server_id = Some(server_id);
        }
    }

    // 6. Lazy creation of Docker network
    let target_docker = resolve_docker_for_server(&state, network.docker_server_id).await?;

    if network.docker_network_id.is_none() {
        let net_name = format!("sb-pn-{}", network_id);
        let (docker_net_id, subnet, gateway) = state
            .provisioner
            .create_private_network(target_docker.as_ref(), &net_name)
            .await?;
        PrivateNetworkRepository::update_docker_network_id(&state.db, network_id, &docker_net_id)
            .await?;
        PrivateNetworkRepository::update_subnet_info(
            &state.db, network_id, subnet.as_deref(), gateway.as_deref(),
        ).await?;
        network.docker_network_id = Some(docker_net_id);
        network.subnet = subnet;
        network.gateway = gateway;
    }

    let docker_net_id = network.docker_network_id.as_ref().unwrap();

    // 7. Attach container
    state
        .provisioner
        .attach_container_to_network(target_docker.as_ref(), container_id, docker_net_id)
        .await?;

    // Record in DB
    PrivateNetworkRepository::add_member(&state.db, network_id, req.database_id).await?;

    // Sync: attach container to active peering bridges for this network
    let peerings = PrivateNetworkRepository::find_peerings_for_network(&state.db, network_id).await?;
    for peering in &peerings {
        if peering.status == "active" {
            if let Some(ref bridge_id) = peering.docker_bridge_id {
                let _ = state.provisioner.attach_container_to_network(
                    target_docker.as_ref(), container_id, bridge_id,
                ).await;
            }
        }
    }

    // Return updated network
    let members =
        PrivateNetworkRepository::find_members_with_db_info(&state.db, network_id).await?;

    Ok(Json(PrivateNetworkResponse {
        id: network.id,
        name: network.name,
        docker_server_id: network.docker_server_id,
        subnet: network.subnet,
        gateway: network.gateway,
        members,
        created_at: network.created_at,
    }))
}

// POST /api/networks/{id}/detach
pub async fn detach_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(network_id): Path<Uuid>,
    Json(req): Json<crate::models::AttachDatabaseRequest>,
) -> AppResult<Json<PrivateNetworkResponse>> {
    let network = PrivateNetworkRepository::find_by_id(&state.db, network_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".to_string()))?;

    if network.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Get the DB to find its container_id
    if let Ok(Some(db)) = DatabaseRepository::find_by_id(&state.db, req.database_id).await {
        if let (Some(ref container_id), Some(ref docker_net_id)) =
            (&db.container_id, &network.docker_network_id)
        {
            let target_docker =
                resolve_docker_for_server(&state, network.docker_server_id).await?;
            let _ = state
                .provisioner
                .detach_container_from_network(target_docker.as_ref(), container_id, docker_net_id)
                .await;

            // Sync: detach from active peering bridges for this network
            let peerings = PrivateNetworkRepository::find_peerings_for_network(&state.db, network_id).await?;
            for peering in &peerings {
                if peering.status == "active" {
                    if let Some(ref bridge_id) = peering.docker_bridge_id {
                        let _ = state.provisioner.detach_container_from_network(
                            target_docker.as_ref(), container_id, bridge_id,
                        ).await;
                    }
                }
            }
        }
    }

    PrivateNetworkRepository::remove_member(&state.db, network_id, req.database_id).await?;

    let members =
        PrivateNetworkRepository::find_members_with_db_info(&state.db, network_id).await?;

    Ok(Json(PrivateNetworkResponse {
        id: network.id,
        name: network.name,
        docker_server_id: network.docker_server_id,
        subnet: network.subnet,
        gateway: network.gateway,
        members,
        created_at: network.created_at,
    }))
}

// --- Peering helpers ---

async fn build_peering_response(
    state: &AppState,
    peering: &crate::models::NetworkPeering,
) -> AppResult<PeeringResponse> {
    let net_a = PrivateNetworkRepository::find_by_id(&state.db, peering.network_a_id)
        .await?
        .ok_or_else(|| AppError::Internal("Network A not found".to_string()))?;
    let net_b = PrivateNetworkRepository::find_by_id(&state.db, peering.network_b_id)
        .await?
        .ok_or_else(|| AppError::Internal("Network B not found".to_string()))?;

    let count_a = PrivateNetworkRepository::count_members(&state.db, peering.network_a_id).await?;
    let count_b = PrivateNetworkRepository::count_members(&state.db, peering.network_b_id).await?;
    let rules = PrivateNetworkRepository::find_rules_by_peering(&state.db, peering.id).await?;

    Ok(PeeringResponse {
        id: peering.id,
        network_a: PeeringNetworkInfo { id: net_a.id, name: net_a.name, member_count: count_a },
        network_b: PeeringNetworkInfo { id: net_b.id, name: net_b.name, member_count: count_b },
        status: peering.status.clone(),
        rules,
        created_at: peering.created_at,
    })
}

// POST /api/peerings
pub async fn create_peering(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<crate::models::CreatePeeringRequest>,
) -> AppResult<Json<PeeringResponse>> {
    if req.network_a_id == req.network_b_id {
        return Err(AppError::BadRequest("Cannot peer a network with itself".to_string()));
    }

    // Both networks must belong to the user
    let net_a = PrivateNetworkRepository::find_by_id(&state.db, req.network_a_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network A not found".to_string()))?;
    if net_a.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let net_b = PrivateNetworkRepository::find_by_id(&state.db, req.network_b_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network B not found".to_string()))?;
    if net_b.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Same Docker server
    match (net_a.docker_server_id, net_b.docker_server_id) {
        (Some(a), Some(b)) if a != b => {
            return Err(AppError::BadRequest("Networks must be on the same Docker server".to_string()));
        }
        _ => {}
    }

    // Check peering doesn't already exist
    if PrivateNetworkRepository::peering_exists(&state.db, req.network_a_id, req.network_b_id).await? {
        return Err(AppError::Conflict("Peering already exists between these networks".to_string()));
    }

    // Check limit
    let count = PrivateNetworkRepository::count_peerings_by_user(&state.db, user.id).await?;
    if count >= MAX_PEERINGS_PER_USER {
        return Err(AppError::BadRequest(format!("Maximum {MAX_PEERINGS_PER_USER} peerings per user")));
    }

    let server_id = net_a.docker_server_id.or(net_b.docker_server_id);
    let peering = PrivateNetworkRepository::create_peering(
        &state.db, user.id, req.network_a_id, req.network_b_id, server_id,
    ).await?;

    // Async provisioning
    let peering_id = peering.id;
    let state_clone = state.clone();
    let net_a_id = req.network_a_id;
    let net_b_id = req.network_b_id;
    tokio::spawn(async move {
        let bridge_name = format!("sb-peer-{}", peering_id);
        let target_docker = match resolve_docker_for_server(&state_clone, server_id).await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Peering {peering_id}: failed to connect to Docker: {e}");
                let _ = PrivateNetworkRepository::update_peering_status(&state_clone.db, peering_id, "error").await;
                return;
            }
        };

        // Create peering bridge with ICC=false
        match state_clone.provisioner.create_peering_network(target_docker.as_ref(), &bridge_name).await {
            Ok(bridge_id) => {
                let _ = PrivateNetworkRepository::update_peering_bridge(&state_clone.db, peering_id, &bridge_id).await;

                // Attach all containers from both networks
                for net_id in [net_a_id, net_b_id] {
                    if let Ok(members) = PrivateNetworkRepository::find_members_with_db_info(&state_clone.db, net_id).await {
                        for member in &members {
                            if let Ok(Some(db)) = DatabaseRepository::find_by_id(&state_clone.db, member.database_id).await {
                                if let Some(ref cid) = db.container_id {
                                    let _ = state_clone.provisioner.attach_container_to_network(
                                        target_docker.as_ref(), cid, &bridge_id,
                                    ).await;
                                }
                            }
                        }
                    }
                }

                let _ = PrivateNetworkRepository::update_peering_status(&state_clone.db, peering_id, "active").await;
                tracing::info!("Peering {peering_id} activated with bridge {bridge_name}");
            }
            Err(e) => {
                tracing::error!("Peering {peering_id}: failed to create bridge: {e}");
                let _ = PrivateNetworkRepository::update_peering_status(&state_clone.db, peering_id, "error").await;
            }
        }
    });

    let resp = build_peering_response(&state, &peering).await?;
    Ok(Json(resp))
}

// GET /api/peerings
pub async fn list_peerings(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> AppResult<Json<Vec<PeeringResponse>>> {
    let peerings = PrivateNetworkRepository::find_peerings_by_user(&state.db, user.id).await?;
    let mut result = Vec::with_capacity(peerings.len());
    for p in &peerings {
        result.push(build_peering_response(&state, p).await?);
    }
    Ok(Json(result))
}

// GET /api/peerings/{id}
pub async fn get_peering(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PeeringResponse>> {
    let peering = PrivateNetworkRepository::find_peering_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peering not found".to_string()))?;
    if peering.user_id != user.id {
        return Err(AppError::Forbidden);
    }
    let resp = build_peering_response(&state, &peering).await?;
    Ok(Json(resp))
}

// DELETE /api/peerings/{id}
pub async fn delete_peering(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let peering = PrivateNetworkRepository::find_peering_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peering not found".to_string()))?;
    if peering.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    if let Some(ref bridge_id) = peering.docker_bridge_id {
        let target_docker = resolve_docker_for_server(&state, peering.docker_server_id).await?;
        let bridge_name = format!("sb-peer-{}", peering.id);

        // Remove iptables
        let _ = state.provisioner.remove_firewall_rules(
            target_docker.as_ref(), &peering.id.to_string(), &bridge_name,
        ).await;

        // Detach all containers from peering bridge
        for net_id in [peering.network_a_id, peering.network_b_id] {
            let members = PrivateNetworkRepository::find_members_with_db_info(&state.db, net_id).await?;
            for member in &members {
                if let Ok(Some(db)) = DatabaseRepository::find_by_id(&state.db, member.database_id).await {
                    if let Some(ref cid) = db.container_id {
                        let _ = state.provisioner.detach_container_from_network(
                            target_docker.as_ref(), cid, bridge_id,
                        ).await;
                    }
                }
            }
        }

        // Remove peering bridge
        let _ = state.provisioner.remove_private_network(target_docker.as_ref(), bridge_id).await;
    }

    PrivateNetworkRepository::delete_peering(&state.db, id).await?;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// POST /api/peerings/{id}/rules
pub async fn create_firewall_rule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(peering_id): Path<Uuid>,
    Json(req): Json<crate::models::CreateFirewallRuleRequest>,
) -> AppResult<Json<crate::models::FirewallRule>> {
    let peering = PrivateNetworkRepository::find_peering_by_id(&state.db, peering_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peering not found".to_string()))?;
    if peering.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    // Validate action
    if req.action != "allow" && req.action != "deny" {
        return Err(AppError::BadRequest("Action must be 'allow' or 'deny'".to_string()));
    }

    // Source/dest must be one of the peering's networks
    if req.source_network_id != peering.network_a_id && req.source_network_id != peering.network_b_id {
        return Err(AppError::BadRequest("Source network must be part of this peering".to_string()));
    }
    if req.dest_network_id != peering.network_a_id && req.dest_network_id != peering.network_b_id {
        return Err(AppError::BadRequest("Destination network must be part of this peering".to_string()));
    }

    // Validate port
    if let Some(port) = req.port {
        if port <= 0 || port >= 65536 {
            return Err(AppError::BadRequest("Port must be between 1 and 65535".to_string()));
        }
    }

    // Validate protocol
    if let Some(ref proto) = req.protocol {
        if proto != "tcp" && proto != "udp" {
            return Err(AppError::BadRequest("Protocol must be 'tcp' or 'udp'".to_string()));
        }
    }

    // Check limit
    let count = PrivateNetworkRepository::count_rules_by_peering(&state.db, peering_id).await?;
    if count >= MAX_RULES_PER_PEERING {
        return Err(AppError::BadRequest(format!("Maximum {MAX_RULES_PER_PEERING} rules per peering")));
    }

    let priority = req.priority.unwrap_or(100);
    let rule = PrivateNetworkRepository::create_firewall_rule(
        &state.db, peering_id, priority, &req.action,
        req.source_network_id, req.dest_network_id,
        req.port, req.protocol.as_deref(), req.description.as_deref(),
    ).await?;

    // Re-apply iptables rules
    if peering.status == "active" {
        apply_peering_iptables(&state, &peering).await?;
    }

    Ok(Json(rule))
}

// DELETE /api/peerings/{peering_id}/rules/{rule_id}
pub async fn delete_firewall_rule(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((peering_id, rule_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let peering = PrivateNetworkRepository::find_peering_by_id(&state.db, peering_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peering not found".to_string()))?;
    if peering.user_id != user.id {
        return Err(AppError::Forbidden);
    }

    let rule = PrivateNetworkRepository::find_rule_by_id(&state.db, rule_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Rule not found".to_string()))?;
    if rule.peering_id != peering_id {
        return Err(AppError::NotFound("Rule not found in this peering".to_string()));
    }

    PrivateNetworkRepository::delete_firewall_rule(&state.db, rule_id).await?;

    // Re-apply iptables rules
    if peering.status == "active" {
        apply_peering_iptables(&state, &peering).await?;
    }

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

/// Re-apply all iptables rules for a peering
async fn apply_peering_iptables(
    state: &AppState,
    peering: &crate::models::NetworkPeering,
) -> AppResult<()> {
    let bridge_name = format!("sb-peer-{}", peering.id);
    let target_docker = resolve_docker_for_server(state, peering.docker_server_id).await?;

    let rules = PrivateNetworkRepository::find_rules_by_peering(&state.db, peering.id).await?;

    // Get subnets for the two networks
    let net_a = PrivateNetworkRepository::find_by_id(&state.db, peering.network_a_id).await?;
    let net_b = PrivateNetworkRepository::find_by_id(&state.db, peering.network_b_id).await?;

    let subnet_a = net_a.as_ref().and_then(|n| n.subnet.clone()).unwrap_or_else(|| "0.0.0.0/0".to_string());
    let subnet_b = net_b.as_ref().and_then(|n| n.subnet.clone()).unwrap_or_else(|| "0.0.0.0/0".to_string());

    let iptables_rules: Vec<crate::services::provisioner::FirewallRule> = rules
        .iter()
        .map(|r| {
            let src = if r.source_network_id == peering.network_a_id { &subnet_a } else { &subnet_b };
            let dst = if r.dest_network_id == peering.network_a_id { &subnet_a } else { &subnet_b };
            (src.clone(), dst.clone(), r.port, r.protocol.clone(), r.action.clone())
        })
        .collect();

    state.provisioner.apply_firewall_rules(
        target_docker.as_ref(),
        &peering.id.to_string(),
        &bridge_name,
        &iptables_rules,
    ).await?;

    Ok(())
}

// GET /api/admin/peerings
pub async fn admin_list_peerings(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PeeringResponse>>> {
    let peerings = PrivateNetworkRepository::list_all_peerings(&state.db).await?;
    let mut result = Vec::with_capacity(peerings.len());
    for p in &peerings {
        result.push(build_peering_response(&state, p).await?);
    }
    Ok(Json(result))
}

// GET /api/admin/networks
pub async fn admin_list_networks(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    let networks = PrivateNetworkRepository::list_all(&state.db).await?;
    let mut result = Vec::with_capacity(networks.len());

    for net in networks {
        let members =
            PrivateNetworkRepository::find_members_with_db_info(&state.db, net.id).await?;
        result.push(serde_json::json!({
            "id": net.id,
            "user_id": net.user_id,
            "name": net.name,
            "docker_server_id": net.docker_server_id,
            "subnet": net.subnet,
            "gateway": net.gateway,
            "member_count": members.len(),
            "members": members,
            "created_at": net.created_at,
        }));
    }

    Ok(Json(result))
}
