-- Docker servers table — remote Docker hosts managed from admin
CREATE TABLE IF NOT EXISTS docker_servers (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(100) NOT NULL,
  url VARCHAR(500) NOT NULL,            -- tcp://1.2.3.4:2376
  tls_ca TEXT,                          -- CA cert PEM (optional)
  tls_cert TEXT,                        -- Client cert PEM (optional)
  tls_key TEXT,                         -- Client key PEM (optional)
  max_containers INTEGER NOT NULL DEFAULT 50,
  active BOOLEAN NOT NULL DEFAULT true,
  region VARCHAR(100),                  -- e.g. "eu-west-1", "paris"
  notes TEXT,
  last_seen_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add server assignment to database_instances
ALTER TABLE database_instances ADD COLUMN IF NOT EXISTS docker_server_id UUID REFERENCES docker_servers(id);

-- Seed default plans — prices slightly elevated, hourly profitable
-- PostgreSQL plans
INSERT INTO plan_templates (name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active)
VALUES
  ('PG Starter',  'postgresql', 0.5, 256,  500,  2, false, true),
  ('PG Pro',      'postgresql', 1.0, 512,  1200, 4, false, true),
  ('PG Business', 'postgresql', 2.0, 1024, 2500, 7, false, true),
  ('PG Enterprise','postgresql',4.0, 2048, 4500, 12, false, true)
ON CONFLICT DO NOTHING;

-- Redis plans
INSERT INTO plan_templates (name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active)
VALUES
  ('Redis Starter',  'redis', 0.25, 128,  300,  1, false, true),
  ('Redis Pro',      'redis', 0.5,  256,  800,  3, false, true),
  ('Redis Business', 'redis', 1.0,  512,  1500, 5, false, true)
ON CONFLICT DO NOTHING;

-- MariaDB plans
INSERT INTO plan_templates (name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active)
VALUES
  ('Maria Starter',  'mariadb', 0.5, 256,  500,  2, false, true),
  ('Maria Pro',      'mariadb', 1.0, 512,  1200, 4, false, true),
  ('Maria Business', 'mariadb', 2.0, 1024, 2500, 7, false, true)
ON CONFLICT DO NOTHING;

-- Bundle plans (PG + Redis together)
INSERT INTO plan_templates (name, db_type, cpu_limit, memory_limit_mb, monthly_price_cents, hourly_price_cents, is_bundle, active)
VALUES
  ('Bundle Starter',  'postgresql', 0.5,  256,  700,  3, true, true),
  ('Bundle Pro',      'postgresql', 1.0,  512,  1800, 6, true, true),
  ('Bundle Business', 'postgresql', 2.0,  1024, 3500, 10, true, true)
ON CONFLICT DO NOTHING;
