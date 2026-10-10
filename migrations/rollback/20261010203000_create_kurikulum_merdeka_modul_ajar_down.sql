-- 20261010203000_create_kurikulum_merdeka_modul_ajar_down.sql
-- Rollback for Kurikulum Merdeka Modul Ajar (RPP) Bounded Context

DROP TRIGGER IF EXISTS trg_cascade_suspend_modul_ajar ON learning_objectives;
DROP FUNCTION IF EXISTS cascade_suspend_modul_ajar_on_tp_demote();
DROP TRIGGER IF EXISTS update_modul_ajar_updated_at ON modul_ajar;
DROP TABLE IF EXISTS modul_ajar CASCADE;
