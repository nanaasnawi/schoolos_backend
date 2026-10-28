-- 20261029000000_remediate_rls_multitenant_and_rapor_ssot_down.sql
-- Description: Rollback script for 20261029000000_remediate_rls_multitenant_and_rapor_ssot.sql

DROP POLICY IF EXISTS kombinasi_mapel_siswa_tenant_isolation ON kombinasi_mapel_siswa;
ALTER TABLE kombinasi_mapel_siswa DISABLE ROW LEVEL SECURITY;

ALTER TABLE kosp_documents ALTER COLUMN establishment_place SET DEFAULT 'Pabuaran';
