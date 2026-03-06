-- Templates de plans (admin-managed)
CREATE TABLE plan_templates (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(100) NOT NULL,
  db_type db_type NOT NULL,
  cpu_limit DOUBLE PRECISION NOT NULL DEFAULT 0.5,
  memory_limit_mb INTEGER NOT NULL DEFAULT 256,
  monthly_price_cents INTEGER NOT NULL,
  hourly_price_cents INTEGER NOT NULL,
  is_bundle BOOLEAN NOT NULL DEFAULT false,
  active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Usage events for start/stop tracking
CREATE TABLE usage_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
  event_type VARCHAR(10) NOT NULL CHECK (event_type IN ('start', 'stop')),
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Monthly billing periods
CREATE TABLE billing_periods (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  period_start TIMESTAMPTZ NOT NULL,
  period_end TIMESTAMPTZ NOT NULL,
  total_cents INTEGER NOT NULL DEFAULT 0,
  stripe_invoice_id VARCHAR(255),
  status VARCHAR(20) NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'invoiced', 'paid', 'failed')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Line items per DB in a billing period
CREATE TABLE billing_line_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  billing_period_id UUID NOT NULL REFERENCES billing_periods(id) ON DELETE CASCADE,
  database_id UUID NOT NULL REFERENCES database_instances(id),
  plan_template_id UUID REFERENCES plan_templates(id),
  hours_used DOUBLE PRECISION NOT NULL,
  amount_cents INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Stripe customer mapping
ALTER TABLE users ADD COLUMN stripe_customer_id VARCHAR(255);

-- Link DB to plan template
ALTER TABLE database_instances ADD COLUMN plan_template_id UUID REFERENCES plan_templates(id);
