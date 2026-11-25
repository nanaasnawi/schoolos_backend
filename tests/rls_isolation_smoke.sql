-- PostgreSQL Fail-Closed RLS Smoke Test
-- Run against a migrated database using a role without BYPASSRLS (e.g., SET LOCAL ROLE app_runtime):
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f backend/tests/rls_isolation_smoke.sql

BEGIN;

DO $$
DECLARE
    v_count BIGINT;
BEGIN
    -- Switch to least-privilege app_runtime role (which has NO BYPASSRLS)
    SET LOCAL ROLE app_runtime;

    -- 1. Without app.current_tenant_id set (and bypass=off), fail-closed RLS must hide all tenant rows
    PERFORM set_config('app.current_tenant_id', '', true);
    PERFORM set_config('app.bypass_rls', 'off', true);

    SELECT COUNT(*) INTO v_count FROM students;
    IF v_count <> 0 THEN
        RAISE EXCEPTION 'FAIL-CLOSED RLS BREACH on students: expected 0 rows when unset, got %', v_count;
    END IF;

    SELECT COUNT(*) INTO v_count FROM users;
    IF v_count <> 0 THEN
        RAISE EXCEPTION 'FAIL-CLOSED RLS BREACH on users: expected 0 rows when unset, got %', v_count;
    END IF;

    SELECT COUNT(*) INTO v_count FROM gradebooks;
    IF v_count <> 0 THEN
        RAISE EXCEPTION 'FAIL-CLOSED RLS BREACH on gradebooks: expected 0 rows when unset, got %', v_count;
    END IF;

    -- 2. Verify app.current_tenant_id = random nonexistent UUID sees 0 rows on all protected tables
    PERFORM set_config('app.current_tenant_id', gen_random_uuid()::text, true);
    SELECT COUNT(*) INTO v_count FROM quiz_attempts;
    IF v_count <> 0 THEN
        RAISE EXCEPTION 'CROSS-TENANT RLS BREACH on quiz_attempts: got % rows for nonexistent tenant', v_count;
    END IF;

    SELECT COUNT(*) INTO v_count FROM refresh_tokens;
    IF v_count <> 0 THEN
        RAISE EXCEPTION 'CROSS-TENANT RLS BREACH on refresh_tokens: got % rows for nonexistent tenant', v_count;
    END IF;
END $$;

ROLLBACK;
