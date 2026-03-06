ALTER TABLE database_instances ADD COLUMN IF NOT EXISTS subdomain VARCHAR(255);
ALTER TABLE database_instances ADD COLUMN IF NOT EXISTS routing_mode VARCHAR(10) DEFAULT 'port';

UPDATE database_instances SET subdomain = LOWER(name) || '-' || LEFT(REPLACE(id::text, '-', ''), 8) || '.db' WHERE subdomain IS NULL;

ALTER TABLE database_instances ALTER COLUMN subdomain SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_database_instances_subdomain ON database_instances (subdomain);

UPDATE database_instances SET tls_mode = 'enabled' WHERE tls_mode = 'disabled'
