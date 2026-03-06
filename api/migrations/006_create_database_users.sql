-- Permission levels for database users
CREATE TYPE db_permission AS ENUM ('admin', 'read_write', 'read_only');

-- Database users with per-instance permissions
CREATE TABLE IF NOT EXISTS database_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
    username VARCHAR(63) NOT NULL,
    password_encrypted TEXT NOT NULL,
    permission db_permission NOT NULL DEFAULT 'read_only',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_database_users_db_username UNIQUE (database_id, username)
);

CREATE INDEX IF NOT EXISTS idx_database_users_database_id ON database_users(database_id)
