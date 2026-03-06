-- Fix billing FK constraints so deleting a DB doesn't destroy usage history
-- Strategy: DROP foreign keys entirely — usage_events and billing_line_items
-- keep the database_id UUID value even after the DB row is deleted.

-- 1. Add denormalized columns to usage_events (for post-deletion lookups)
ALTER TABLE usage_events
  ADD COLUMN IF NOT EXISTS user_id UUID,
  ADD COLUMN IF NOT EXISTS database_name VARCHAR(100),
  ADD COLUMN IF NOT EXISTS plan_template_id UUID;

-- 2. Backfill from database_instances
UPDATE usage_events ue
SET
  user_id = di.user_id,
  database_name = di.name,
  plan_template_id = di.plan_template_id
FROM database_instances di
WHERE ue.database_id = di.id
  AND ue.user_id IS NULL;

-- 3. Drop FK on usage_events.database_id (was ON DELETE CASCADE)
ALTER TABLE usage_events
  DROP CONSTRAINT IF EXISTS usage_events_database_id_fkey;

-- 4. Drop FK on billing_line_items.database_id (was no action → FK error on delete)
ALTER TABLE billing_line_items
  DROP CONSTRAINT IF EXISTS billing_line_items_database_id_fkey;
