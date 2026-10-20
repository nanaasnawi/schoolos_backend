-- 20261021000000_enforce_postgresql_rls_all_tables_down.sql
-- Description: Rollback script for PostgreSQL Row Level Security (RLS)

DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN (
        SELECT DISTINCT c.table_name 
        FROM information_schema.columns c
        JOIN information_schema.tables t ON c.table_name = t.table_name AND t.table_schema = 'public'
        WHERE c.column_name = 'tenant_id' 
          AND c.table_schema = 'public'
          AND t.table_type = 'BASE TABLE'
    ) LOOP
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_isolation ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_modify ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_public_read ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_auth_lookup ON %I', r.table_name, r.table_name);
        EXECUTE format('ALTER TABLE %I DISABLE ROW LEVEL SECURITY', r.table_name);
    END LOOP;
END;
$$;

DROP FUNCTION IF EXISTS is_tenant_allowed(UUID);
