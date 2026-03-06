CREATE TABLE migration_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
    filename VARCHAR(255) NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(database_id, filename)
);

CREATE INDEX idx_migration_records_database_id ON migration_records(database_id);
