-- Add subnet and gateway info to private networks
ALTER TABLE private_networks ADD COLUMN IF NOT EXISTS subnet VARCHAR(43);
ALTER TABLE private_networks ADD COLUMN IF NOT EXISTS gateway VARCHAR(39);
