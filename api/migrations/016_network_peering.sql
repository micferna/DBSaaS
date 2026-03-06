CREATE TABLE IF NOT EXISTS network_peerings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    network_a_id UUID NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
    network_b_id UUID NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
    docker_bridge_id VARCHAR(128),
    docker_server_id UUID REFERENCES docker_servers(id),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT peering_different CHECK (network_a_id <> network_b_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_peering_unique
    ON network_peerings (LEAST(network_a_id, network_b_id), GREATEST(network_a_id, network_b_id));

CREATE TABLE IF NOT EXISTS firewall_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    peering_id UUID NOT NULL REFERENCES network_peerings(id) ON DELETE CASCADE,
    priority INT NOT NULL DEFAULT 100,
    action VARCHAR(5) NOT NULL DEFAULT 'deny' CHECK (action IN ('allow', 'deny')),
    source_network_id UUID NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
    dest_network_id UUID NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
    port INT CHECK (port IS NULL OR (port > 0 AND port < 65536)),
    protocol VARCHAR(4) DEFAULT 'tcp' CHECK (protocol IS NULL OR protocol IN ('tcp', 'udp')),
    description VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
)
