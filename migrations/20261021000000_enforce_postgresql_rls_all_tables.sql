-- 20261021000000_enforce_postgresql_rls_all_tables.sql
-- Description: Enforce PostgreSQL Row Level Security (RLS) across all multi-tenant tables
-- Complies with ADR-0004 (Multi-Tenant RLS) & ADR-0013 (Single Source of Truth)
-- Includes Fail-Safe Transition Protection for PKBM As-Salafiyah (Live Production)

-- 1. Helper function for fail-safe tenant isolation
CREATE OR REPLACE FUNCTION is_tenant_allowed(record_tenant_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    curr_tenant TEXT;
    bypass TEXT;
    as_salafiyah_id UUID;
BEGIN
    -- [Rule 1] System Worker / SuperAdmin Bypass (SmartReminder, OutboxDispatcher, Migrations)
    bypass := current_setting('app.bypass_rls', true);
    IF bypass = 'on' THEN
        RETURN true;
    END IF;

    -- [Rule 2] Verified Multi-Tenant Session Context
    curr_tenant := NULLIF(current_setting('app.current_tenant_id', true), '');
    IF curr_tenant IS NOT NULL THEN
        RETURN record_tenant_id = curr_tenant::uuid;
    END IF;

    -- [Rule 3] Production Safety Net for PKBM As-Salafiyah:
    -- If no session variable is set (legacy uninstrumented query), permit As-Salafiyah tenant
    -- so production operations never experience blank screens / 0 rows during or after maintenance.
    SELECT id INTO as_salafiyah_id FROM tenants WHERE name ILIKE '%As-Salafiyah%' LIMIT 1;
    IF as_salafiyah_id IS NOT NULL AND record_tenant_id = as_salafiyah_id THEN
        RETURN true;
    END IF;

    -- If no match, deny access (prevent cross-tenant data leakage)
    RETURN false;
END;
$$ LANGUAGE plpgsql STABLE SECURITY DEFINER;

-- 2. Dynamically enable RLS and attach isolation policy to all tenant-scoped tables
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
          AND c.table_name NOT IN ('tenants', 'schools', 'users', 'roles', 'library_books')
    ) LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_isolation ON %I', r.table_name, r.table_name);
        EXECUTE format('CREATE POLICY %I_tenant_isolation ON %I FOR ALL USING (is_tenant_allowed(tenant_id)) WITH CHECK (is_tenant_allowed(tenant_id))', r.table_name, r.table_name);
    END LOOP;
END;
$$;

-- 3. Dedicated Policies for Special & Pre-Auth Tables

-- ROLES: System default roles are shared across all tenants, custom roles are tenant-scoped
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'roles' AND table_schema = 'public') THEN
        ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
        DROP POLICY IF EXISTS roles_tenant_isolation ON roles;
        CREATE POLICY roles_tenant_isolation ON roles 
            FOR ALL 
            USING (is_system_default = true OR is_tenant_allowed(tenant_id))
            WITH CHECK (is_tenant_allowed(tenant_id));
    END IF;
END;
$$;

-- LIBRARY BOOKS: Global SIBI Kemdikdasmen books are publicly readable by all students/teachers
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'library_books' AND table_schema = 'public') THEN
        ALTER TABLE library_books ENABLE ROW LEVEL SECURITY;
        DROP POLICY IF EXISTS library_books_public_read ON library_books;
        CREATE POLICY library_books_public_read ON library_books 
            FOR SELECT 
            USING (true);
        DROP POLICY IF EXISTS library_books_tenant_isolation ON library_books;
        CREATE POLICY library_books_tenant_isolation ON library_books 
            FOR ALL 
            USING (tenant_id IS NULL OR is_tenant_allowed(tenant_id))
            WITH CHECK (tenant_id IS NULL OR is_tenant_allowed(tenant_id));
    END IF;
END;
$$;

-- SCHOOLS: Public read for login & school info, restricted modification
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'schools' AND table_schema = 'public') THEN
        ALTER TABLE schools ENABLE ROW LEVEL SECURITY;
        DROP POLICY IF EXISTS schools_public_read ON schools;
        CREATE POLICY schools_public_read ON schools 
            FOR SELECT 
            USING (true);
        DROP POLICY IF EXISTS schools_tenant_modify ON schools;
        CREATE POLICY schools_tenant_modify ON schools 
            FOR ALL 
            USING (is_tenant_allowed(tenant_id))
            WITH CHECK (is_tenant_allowed(tenant_id));
    END IF;
END;
$$;

-- USERS: Public select by username during authentication handshake, restricted modification
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'users' AND table_schema = 'public') THEN
        ALTER TABLE users ENABLE ROW LEVEL SECURITY;
        DROP POLICY IF EXISTS users_auth_lookup ON users;
        CREATE POLICY users_auth_lookup ON users 
            FOR SELECT 
            USING (true);
        DROP POLICY IF EXISTS users_tenant_modify ON users;
        CREATE POLICY users_tenant_modify ON users 
            FOR ALL 
            USING (is_tenant_allowed(tenant_id))
            WITH CHECK (is_tenant_allowed(tenant_id));
    END IF;
END;
$$;
