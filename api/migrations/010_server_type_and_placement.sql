-- Add server_type to distinguish platform servers from client servers
ALTER TABLE docker_servers ADD COLUMN IF NOT EXISTS server_type VARCHAR(20) NOT NULL DEFAULT 'client'
  CHECK (server_type IN ('platform', 'client'));
