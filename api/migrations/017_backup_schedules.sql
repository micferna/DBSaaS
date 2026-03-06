CREATE TABLE backup_schedules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  database_id UUID NOT NULL UNIQUE REFERENCES database_instances(id) ON DELETE CASCADE,
  interval_hours INT NOT NULL DEFAULT 24,
  retention_count INT NOT NULL DEFAULT 7,
  enabled BOOLEAN NOT NULL DEFAULT true,
  last_run_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
)