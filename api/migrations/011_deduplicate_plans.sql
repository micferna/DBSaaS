-- Deduplicate plan_templates: keep only the oldest row per (name, db_type, is_bundle)
DELETE FROM plan_templates
WHERE id NOT IN (
  SELECT DISTINCT ON (name, db_type, is_bundle) id
  FROM plan_templates
  ORDER BY name, db_type, is_bundle, created_at ASC
);

-- Add unique constraint so ON CONFLICT DO NOTHING works in seed migration
ALTER TABLE plan_templates ADD CONSTRAINT plan_templates_name_type_bundle_uniq UNIQUE (name, db_type, is_bundle);
