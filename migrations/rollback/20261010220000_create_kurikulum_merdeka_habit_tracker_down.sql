-- 20261010220000_create_kurikulum_merdeka_habit_tracker_down.sql
-- Rollback for habit_tracker_entries migration

DROP TRIGGER IF EXISTS trg_calculate_habit_metrics ON habit_tracker_entries;
DROP FUNCTION IF EXISTS fn_calculate_habit_metrics();
DROP FUNCTION IF EXISTS fn_expire_stale_habit_entries(UUID, INT);
DROP TABLE IF EXISTS habit_tracker_entries CASCADE;
