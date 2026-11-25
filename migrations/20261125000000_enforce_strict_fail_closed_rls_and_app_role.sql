-- 20261125000000_enforce_strict_fail_closed_rls_and_app_role.sql
-- Security Remediation: Fail-Closed Multi-Tenant PostgreSQL RLS + Runtime Table Consolidation + Child Table Policies

-- 1. Consolidate runtime-created tables/columns so runtime requests (and app_runtime role) never execute DDL.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS avatar_url TEXT,
    ADD COLUMN IF NOT EXISTS avatar_data BYTEA,
    ADD COLUMN IF NOT EXISTS avatar_mime VARCHAR(50);

ALTER TABLE student_documents
    ADD COLUMN IF NOT EXISTS file_data BYTEA;

ALTER TABLE academic_years
    ADD COLUMN IF NOT EXISTS current_semester SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS report_date DATE;

ALTER TABLE schools
    ADD COLUMN IF NOT EXISTS headmaster_name VARCHAR(255),
    ADD COLUMN IF NOT EXISTS headmaster_nip VARCHAR(50),
    ADD COLUMN IF NOT EXISTS village VARCHAR(100),
    ADD COLUMN IF NOT EXISTS district VARCHAR(100),
    ADD COLUMN IF NOT EXISTS regency VARCHAR(100),
    ADD COLUMN IF NOT EXISTS province VARCHAR(100),
    ADD COLUMN IF NOT EXISTS postal_code VARCHAR(20),
    ADD COLUMN IF NOT EXISTS website VARCHAR(255);

ALTER TABLE inquiry_messages
    ADD COLUMN IF NOT EXISTS attachment_url TEXT,
    ADD COLUMN IF NOT EXISTS attachment_type VARCHAR(50);

ALTER TABLE quiz_questions
    ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL;

ALTER TABLE attempt_answers
    ADD COLUMN IF NOT EXISTS teacher_feedback TEXT DEFAULT NULL;

ALTER TABLE quizzes
    ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';

ALTER TABLE assignments
    ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';

ALTER TABLE assignment_questions
    ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL;

ALTER TABLE pedagogical_assessments
    ADD COLUMN IF NOT EXISTS source_type VARCHAR(32) DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS source_id UUID DEFAULT NULL;

ALTER TABLE gradebook_snapshots
    ADD COLUMN IF NOT EXISTS ats_score NUMERIC(5,2) DEFAULT NULL;

CREATE TABLE IF NOT EXISTS uploaded_files (
    filename VARCHAR(255) PRIMARY KEY,
    mime_type VARCHAR(100) NOT NULL,
    file_data BYTEA NOT NULL,
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    is_public BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

ALTER TABLE uploaded_files
    ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS is_public BOOLEAN NOT NULL DEFAULT true;

CREATE TABLE IF NOT EXISTS quiz_pre_start_notices (
    quiz_id UUID PRIMARY KEY REFERENCES quizzes(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    start_bucket VARCHAR(32) NOT NULL,
    notified_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS quiz_executive_pre_start_batches (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_bucket VARCHAR(32) NOT NULL,
    sent_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, user_id, start_bucket)
);

CREATE TABLE IF NOT EXISTS student_shiba_pets (
    student_id UUID PRIMARY KEY REFERENCES students(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    pet_stage INT NOT NULL DEFAULT 1,
    food_points INT NOT NULL DEFAULT 25,
    total_fed INT NOT NULL DEFAULT 0,
    biscuits INT NOT NULL DEFAULT 2,
    fifty_fifty_tokens INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS daily_quest_chest_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    claim_date DATE NOT NULL,
    reward_type VARCHAR(40) NOT NULL,
    reward_amount INT NOT NULL DEFAULT 0,
    reward_label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_daily_quest_chest_claim UNIQUE (student_id, claim_date)
);

CREATE TABLE IF NOT EXISTS daily_quest_bonus_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    bonus_date DATE NOT NULL,
    biscuits_earned INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Redefine is_tenant_allowed(UUID) as FAIL-CLOSED:
--    Unset app.current_tenant_id MUST return FALSE unless app.bypass_rls = 'on'.
CREATE OR REPLACE FUNCTION is_tenant_allowed(record_tenant_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    curr_tenant TEXT;
BEGIN
    IF current_setting('app.bypass_rls', true) = 'on' THEN
        RETURN true;
    END IF;

    curr_tenant := NULLIF(BTRIM(COALESCE(current_setting('app.current_tenant_id', true), '')), '');
    IF curr_tenant IS NOT NULL
       AND curr_tenant ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
       AND record_tenant_id IS NOT NULL THEN
        RETURN record_tenant_id = curr_tenant::uuid;
    END IF;

    RETURN false;
END;
$$ LANGUAGE plpgsql STABLE;

-- 3. Narrow SECURITY DEFINER login lookup helper so `users` no longer needs permissive USING (true) policy.
CREATE OR REPLACE FUNCTION auth_lookup_users_by_login(p_identifier TEXT)
RETURNS TABLE (
    id UUID,
    tenant_id UUID,
    username VARCHAR(100),
    email VARCHAR(255),
    password_hash VARCHAR(255),
    full_name VARCHAR(255),
    is_active BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        u.id,
        u.tenant_id,
        u.username,
        u.email,
        u.password_hash,
        u.full_name,
        u.is_active
    FROM users u
    WHERE LOWER(BTRIM(u.email)) = LOWER(BTRIM(p_identifier))
       OR (u.username IS NOT NULL AND LOWER(BTRIM(u.username)) = LOWER(BTRIM(p_identifier)))
    LIMIT 25;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER SET search_path = public;

-- 4. Apply strict ENABLE + FORCE ROW LEVEL SECURITY on all tables with `tenant_id`.
DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN (
        SELECT DISTINCT c.table_name
        FROM information_schema.columns c
        JOIN information_schema.tables t
          ON c.table_name = t.table_name
         AND t.table_schema = 'public'
        WHERE c.column_name = 'tenant_id'
          AND c.table_schema = 'public'
          AND t.table_type = 'BASE TABLE'
          AND c.table_name NOT IN (
              'roles',
              'users',
              'academic_calendar_events',
              'learning_outcomes',
              'learning_objectives',
              'learning_objective_flows',
              'library_books',
              'uploaded_files'
          )
    ) LOOP
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_isolation ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_modify ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_tenant_select ON %I', r.table_name, r.table_name);
        EXECUTE format('DROP POLICY IF EXISTS %I_public_read ON %I', r.table_name, r.table_name);
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', r.table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', r.table_name);
        EXECUTE format(
            'CREATE POLICY %I_tenant_isolation ON %I FOR ALL USING (is_tenant_allowed(tenant_id)) WITH CHECK (is_tenant_allowed(tenant_id))',
            r.table_name, r.table_name
        );
    END LOOP;
END;
$$;

-- 5. Users table: strictly tenant-isolated (remove permissive users_auth_lookup USING (true)).
DROP POLICY IF EXISTS users_auth_lookup ON users;
DROP POLICY IF EXISTS users_tenant_modify ON users;
DROP POLICY IF EXISTS users_tenant_isolation ON users;
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY users_tenant_isolation ON users
    FOR ALL
    USING (is_tenant_allowed(tenant_id))
    WITH CHECK (is_tenant_allowed(tenant_id));

-- 6. Shared system-default or nullable global catalog tables:
--    Read allowed when system/global baseline, writes strictly scoped to active tenant.
DROP POLICY IF EXISTS roles_tenant_isolation ON roles;
DROP POLICY IF EXISTS roles_tenant_select ON roles;
DROP POLICY IF EXISTS roles_tenant_modify ON roles;
ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE roles FORCE ROW LEVEL SECURITY;
CREATE POLICY roles_tenant_select ON roles
    FOR SELECT
    USING (is_system_default = true OR is_tenant_allowed(tenant_id));
CREATE POLICY roles_tenant_modify ON roles
    FOR ALL
    USING (is_tenant_allowed(tenant_id))
    WITH CHECK (is_tenant_allowed(tenant_id));

DO $$
DECLARE
    tbl TEXT;
BEGIN
    FOREACH tbl IN ARRAY ARRAY[
        'academic_calendar_events',
        'learning_outcomes',
        'learning_objectives',
        'learning_objective_flows',
        'library_books'
    ] LOOP
        IF to_regclass(format('public.%I', tbl)) IS NOT NULL THEN
            EXECUTE format('DROP POLICY IF EXISTS %I_tenant_isolation ON %I', tbl, tbl);
            EXECUTE format('DROP POLICY IF EXISTS %I_tenant_select ON %I', tbl, tbl);
            EXECUTE format('DROP POLICY IF EXISTS %I_tenant_modify ON %I', tbl, tbl);
            EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tbl);
            EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', tbl);
            EXECUTE format(
                'CREATE POLICY %I_tenant_select ON %I FOR SELECT USING (tenant_id IS NULL OR is_tenant_allowed(tenant_id))',
                tbl, tbl
            );
            EXECUTE format(
                'CREATE POLICY %I_tenant_modify ON %I FOR INSERT WITH CHECK (is_tenant_allowed(tenant_id))',
                tbl, tbl
            );
            EXECUTE format(
                'DROP POLICY IF EXISTS %I_tenant_update ON %I',
                tbl, tbl
            );
            EXECUTE format(
                'CREATE POLICY %I_tenant_update ON %I FOR UPDATE USING (is_tenant_allowed(tenant_id)) WITH CHECK (is_tenant_allowed(tenant_id))',
                tbl, tbl
            );
            EXECUTE format(
                'DROP POLICY IF EXISTS %I_tenant_delete ON %I',
                tbl, tbl
            );
            EXECUTE format(
                'CREATE POLICY %I_tenant_delete ON %I FOR DELETE USING (is_tenant_allowed(tenant_id))',
                tbl, tbl
            );
        END IF;
    END LOOP;
END;
$$;

-- uploaded_files: allow reading public learning assets or tenant-scoped files; write strictly when scoped or public AI assets
DROP POLICY IF EXISTS uploaded_files_select ON uploaded_files;
DROP POLICY IF EXISTS uploaded_files_write ON uploaded_files;
ALTER TABLE uploaded_files ENABLE ROW LEVEL SECURITY;
ALTER TABLE uploaded_files FORCE ROW LEVEL SECURITY;
CREATE POLICY uploaded_files_select ON uploaded_files
    FOR SELECT
    USING (is_public = true OR is_tenant_allowed(tenant_id));
CREATE POLICY uploaded_files_write ON uploaded_files
    FOR ALL
    USING (is_tenant_allowed(tenant_id) OR current_setting('app.bypass_rls', true) = 'on')
    WITH CHECK (is_tenant_allowed(tenant_id) OR (tenant_id IS NULL AND is_public = true) OR current_setting('app.bypass_rls', true) = 'on');

-- 7. Root/system tables without `tenant_id`
DROP POLICY IF EXISTS tenants_isolation ON tenants;
ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
CREATE POLICY tenants_isolation ON tenants
    FOR SELECT
    USING (
        current_setting('app.bypass_rls', true) = 'on'
        OR id::text = NULLIF(BTRIM(COALESCE(current_setting('app.current_tenant_id', true), '')), '')
    );
DROP POLICY IF EXISTS tenants_write ON tenants;
DROP POLICY IF EXISTS tenants_insert_delete ON tenants;
DROP POLICY IF EXISTS tenants_update_own ON tenants;
DROP POLICY IF EXISTS tenants_insert_system ON tenants;
DROP POLICY IF EXISTS tenants_delete_system ON tenants;
CREATE POLICY tenants_update_own ON tenants
    FOR UPDATE
    USING (is_tenant_allowed(id))
    WITH CHECK (is_tenant_allowed(id));
CREATE POLICY tenants_insert_system ON tenants
    FOR INSERT
    WITH CHECK (current_setting('app.bypass_rls', true) = 'on');
CREATE POLICY tenants_delete_system ON tenants
    FOR DELETE
    USING (current_setting('app.bypass_rls', true) = 'on');

DROP POLICY IF EXISTS system_settings_read ON system_settings;
DROP POLICY IF EXISTS system_settings_write ON system_settings;
ALTER TABLE system_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE system_settings FORCE ROW LEVEL SECURITY;
CREATE POLICY system_settings_read ON system_settings
    FOR SELECT
    USING (key = 'maintenance' OR current_setting('app.bypass_rls', true) = 'on');
CREATE POLICY system_settings_write ON system_settings
    FOR ALL
    USING (current_setting('app.bypass_rls', true) = 'on')
    WITH CHECK (current_setting('app.bypass_rls', true) = 'on');

-- 8. Child / join tables without direct `tenant_id` column: enforce parent-join tenant isolation + FORCE RLS.
DROP POLICY IF EXISTS quiz_questions_parent_tenant ON quiz_questions;
ALTER TABLE quiz_questions ENABLE ROW LEVEL SECURITY;
ALTER TABLE quiz_questions FORCE ROW LEVEL SECURITY;
CREATE POLICY quiz_questions_parent_tenant ON quiz_questions
    FOR ALL
    USING (EXISTS (SELECT 1 FROM quizzes q WHERE q.id = quiz_questions.quiz_id AND is_tenant_allowed(q.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM quizzes q WHERE q.id = quiz_questions.quiz_id AND is_tenant_allowed(q.tenant_id)));

DROP POLICY IF EXISTS quiz_choices_parent_tenant ON quiz_choices;
ALTER TABLE quiz_choices ENABLE ROW LEVEL SECURITY;
ALTER TABLE quiz_choices FORCE ROW LEVEL SECURITY;
CREATE POLICY quiz_choices_parent_tenant ON quiz_choices
    FOR ALL
    USING (EXISTS (
        SELECT 1 FROM quiz_questions qq
        JOIN quizzes q ON q.id = qq.quiz_id
        WHERE qq.id = quiz_choices.question_id AND is_tenant_allowed(q.tenant_id)
    ))
    WITH CHECK (EXISTS (
        SELECT 1 FROM quiz_questions qq
        JOIN quizzes q ON q.id = qq.quiz_id
        WHERE qq.id = quiz_choices.question_id AND is_tenant_allowed(q.tenant_id)
    ));

DROP POLICY IF EXISTS attempt_answers_parent_tenant ON attempt_answers;
ALTER TABLE attempt_answers ENABLE ROW LEVEL SECURITY;
ALTER TABLE attempt_answers FORCE ROW LEVEL SECURITY;
CREATE POLICY attempt_answers_parent_tenant ON attempt_answers
    FOR ALL
    USING (EXISTS (SELECT 1 FROM quiz_attempts qa WHERE qa.id = attempt_answers.attempt_id AND is_tenant_allowed(qa.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM quiz_attempts qa WHERE qa.id = attempt_answers.attempt_id AND is_tenant_allowed(qa.tenant_id)));

DROP POLICY IF EXISTS assignment_question_choices_parent_tenant ON assignment_question_choices;
ALTER TABLE assignment_question_choices ENABLE ROW LEVEL SECURITY;
ALTER TABLE assignment_question_choices FORCE ROW LEVEL SECURITY;
CREATE POLICY assignment_question_choices_parent_tenant ON assignment_question_choices
    FOR ALL
    USING (EXISTS (SELECT 1 FROM assignment_questions aq WHERE aq.id = assignment_question_choices.question_id AND is_tenant_allowed(aq.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM assignment_questions aq WHERE aq.id = assignment_question_choices.question_id AND is_tenant_allowed(aq.tenant_id)));

DROP POLICY IF EXISTS assignment_submission_answers_parent_tenant ON assignment_submission_answers;
ALTER TABLE assignment_submission_answers ENABLE ROW LEVEL SECURITY;
ALTER TABLE assignment_submission_answers FORCE ROW LEVEL SECURITY;
CREATE POLICY assignment_submission_answers_parent_tenant ON assignment_submission_answers
    FOR ALL
    USING (EXISTS (SELECT 1 FROM assignment_submissions sub WHERE sub.id = assignment_submission_answers.submission_id AND is_tenant_allowed(sub.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM assignment_submissions sub WHERE sub.id = assignment_submission_answers.submission_id AND is_tenant_allowed(sub.tenant_id)));

DROP POLICY IF EXISTS assessment_rule_components_parent_tenant ON assessment_rule_components;
ALTER TABLE assessment_rule_components ENABLE ROW LEVEL SECURITY;
ALTER TABLE assessment_rule_components FORCE ROW LEVEL SECURITY;
CREATE POLICY assessment_rule_components_parent_tenant ON assessment_rule_components
    FOR ALL
    USING (EXISTS (SELECT 1 FROM assessment_rules ar WHERE ar.id = assessment_rule_components.rule_id AND is_tenant_allowed(ar.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM assessment_rules ar WHERE ar.id = assessment_rule_components.rule_id AND is_tenant_allowed(ar.tenant_id)));

DROP POLICY IF EXISTS kosp_committees_parent_tenant ON kosp_committees;
ALTER TABLE kosp_committees ENABLE ROW LEVEL SECURITY;
ALTER TABLE kosp_committees FORCE ROW LEVEL SECURITY;
CREATE POLICY kosp_committees_parent_tenant ON kosp_committees
    FOR ALL
    USING (EXISTS (SELECT 1 FROM kosp_documents kd WHERE kd.id = kosp_committees.kosp_document_id AND is_tenant_allowed(kd.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM kosp_documents kd WHERE kd.id = kosp_committees.kosp_document_id AND is_tenant_allowed(kd.tenant_id)));

DROP POLICY IF EXISTS kosp_signatories_parent_tenant ON kosp_signatories;
ALTER TABLE kosp_signatories ENABLE ROW LEVEL SECURITY;
ALTER TABLE kosp_signatories FORCE ROW LEVEL SECURITY;
CREATE POLICY kosp_signatories_parent_tenant ON kosp_signatories
    FOR ALL
    USING (EXISTS (SELECT 1 FROM kosp_documents kd WHERE kd.id = kosp_signatories.kosp_document_id AND is_tenant_allowed(kd.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM kosp_documents kd WHERE kd.id = kosp_signatories.kosp_document_id AND is_tenant_allowed(kd.tenant_id)));

DROP POLICY IF EXISTS user_roles_parent_tenant ON user_roles;
ALTER TABLE user_roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_roles FORCE ROW LEVEL SECURITY;
CREATE POLICY user_roles_parent_tenant ON user_roles
    FOR ALL
    USING (EXISTS (SELECT 1 FROM users u WHERE u.id = user_roles.user_id AND is_tenant_allowed(u.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM users u WHERE u.id = user_roles.user_id AND is_tenant_allowed(u.tenant_id)));

DROP POLICY IF EXISTS role_permissions_select ON role_permissions;
DROP POLICY IF EXISTS role_permissions_modify ON role_permissions;
ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;
ALTER TABLE role_permissions FORCE ROW LEVEL SECURITY;
CREATE POLICY role_permissions_select ON role_permissions
    FOR SELECT
    USING (EXISTS (
        SELECT 1 FROM roles r
        WHERE r.id = role_permissions.role_id
          AND (r.is_system_default = true OR is_tenant_allowed(r.tenant_id))
    ));
CREATE POLICY role_permissions_modify ON role_permissions
    FOR ALL
    USING (EXISTS (SELECT 1 FROM roles r WHERE r.id = role_permissions.role_id AND is_tenant_allowed(r.tenant_id)))
    WITH CHECK (EXISTS (SELECT 1 FROM roles r WHERE r.id = role_permissions.role_id AND is_tenant_allowed(r.tenant_id)));

-- 9. Provision non-superuser / NOBYPASSRLS runtime role (`app_runtime`) if permitted.
--    Note: Operators set LOGIN + password via `ALTER ROLE app_runtime WITH LOGIN PASSWORD '...';`
--    on Railway so no credentials are stored in repository migrations.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_runtime') THEN
        CREATE ROLE app_runtime NOLOGIN NOINHERIT NOSUPERUSER NOBYPASSRLS;
    ELSE
        ALTER ROLE app_runtime NOINHERIT NOSUPERUSER NOBYPASSRLS;
    END IF;
    GRANT USAGE ON SCHEMA public TO app_runtime;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO app_runtime;
    GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO app_runtime;
    GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO app_runtime;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO app_runtime;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO app_runtime;
EXCEPTION
    WHEN insufficient_privilege THEN
        RAISE NOTICE 'Skipping app_runtime role creation/grants due to insufficient_privilege';
END;
$$;
