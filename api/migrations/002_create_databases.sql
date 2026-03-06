CREATE TYPE db_type AS ENUM ('postgresql', 'redis');
CREATE TYPE db_status AS ENUM ('provisioning', 'running', 'stopped', 'error', 'deleting');

CREATE TABLE database_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(63) NOT NULL,
    db_type db_type NOT NULL,
    status db_status NOT NULL DEFAULT 'provisioning',
    container_id VARCHAR(128),
    network_id VARCHAR(128),
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL,
    username VARCHAR(63) NOT NULL,
    password_encrypted TEXT NOT NULL,
    database_name VARCHAR(63),
    tls_cert TEXT,
    cpu_limit DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    memory_limit_mb INTEGER NOT NULL DEFAULT 256,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, name)
);

CREATE INDEX idx_databases_user_id ON database_instances(user_id);
CREATE INDEX idx_databases_status ON database_instances(status);
CREATE INDEX idx_databases_port ON database_instances(port);
