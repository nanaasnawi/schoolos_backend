-- 20260912204500_add_dapodik_sync_audit_to_schools.sql
-- Add audit trail columns for tracking when and who last pulled Dapodik data

ALTER TABLE schools ADD COLUMN IF NOT EXISTS dapodik_last_synced_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE schools ADD COLUMN IF NOT EXISTS dapodik_last_synced_by VARCHAR(255);
