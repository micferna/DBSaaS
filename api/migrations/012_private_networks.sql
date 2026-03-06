CREATE TABLE IF NOT EXISTS private_networks (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name VARCHAR(63) NOT NULL,
  docker_network_id VARCHAR(128),
  docker_server_id UUID REFERENCES docker_servers(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(user_id, name)
);

CREATE TABLE IF NOT EXISTS private_network_members (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  network_id UUID NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
  database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
  joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(network_id, database_id)
)
