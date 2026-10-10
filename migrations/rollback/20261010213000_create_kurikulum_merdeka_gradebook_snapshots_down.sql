-- 20261010213000_create_kurikulum_merdeka_gradebook_snapshots_down.sql
-- Rollback for gradebook_snapshots migration

DROP TRIGGER IF EXISTS update_gradebook_snapshots_updated_at ON gradebook_snapshots;
DROP TABLE IF EXISTS gradebook_snapshots CASCADE;
