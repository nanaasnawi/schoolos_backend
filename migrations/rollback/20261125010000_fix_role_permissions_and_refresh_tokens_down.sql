-- 20261125010000_fix_role_permissions_and_refresh_tokens_down.sql
DROP POLICY IF EXISTS refresh_tokens_tenant_isolation ON refresh_tokens;
DROP TABLE IF EXISTS refresh_tokens;
