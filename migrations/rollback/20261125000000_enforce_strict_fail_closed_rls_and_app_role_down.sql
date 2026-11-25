-- 20261125000000_enforce_strict_fail_closed_rls_and_app_role_down.sql
-- Emergency Rollback for Fail-Closed RLS Migration

DROP FUNCTION IF EXISTS auth_lookup_users_by_login(TEXT);

-- Restore previous permissive is_tenant_allowed() behavior only if rolling back to un-updated backend binary
CREATE OR REPLACE FUNCTION is_tenant_allowed(record_tenant_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    curr_tenant TEXT;
BEGIN
    IF current_setting('app.bypass_rls', true) = 'on' THEN
        RETURN true;
    END IF;
    curr_tenant := NULLIF(current_setting('app.current_tenant_id', true), '');
    IF curr_tenant IS NOT NULL THEN
        RETURN record_tenant_id = curr_tenant::uuid;
    END IF;
    RETURN true;
END;
$$ LANGUAGE plpgsql STABLE;

DROP POLICY IF EXISTS users_tenant_isolation ON users;
CREATE POLICY users_auth_lookup ON users FOR SELECT USING (true);
CREATE POLICY users_tenant_modify ON users FOR ALL USING (is_tenant_allowed(tenant_id)) WITH CHECK (is_tenant_allowed(tenant_id));
