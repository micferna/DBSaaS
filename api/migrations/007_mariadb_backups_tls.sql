-- Add MariaDB to db_type enum
ALTER TYPE db_type ADD VALUE IF NOT EXISTS 'mariadb';

-- Backups table
CREATE TABLE IF NOT EXISTS database_backups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
    filename VARCHAR(255) NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_database_backups_database_id ON database_backups(database_id);

-- TLS mode: 'enabled' (default for backward compat) or 'disabled'
ALTER TABLE database_instances ADD COLUMN IF NOT EXISTS tls_mode VARCHAR(16) NOT NULL DEFAULT 'enabled'
