-- 20261125010000_fix_role_permissions_and_refresh_tokens.sql
-- 1. Enforce strict least-privilege RBAC on role_permissions (revoke Teacher.Read/Student.Read from Students & Parents;
--    revoke admin-level permissions from non-admin Teachers/Tutors).
-- 2. Create `refresh_tokens` table with reuse-detection family tracking & FORCE ROW LEVEL SECURITY.

DELETE FROM role_permissions rp
USING roles r
WHERE rp.role_id = r.id
  AND LOWER(BTRIM(r.name)) IN (
      'siswa', 'student', 'murid',
      'wali siswa', 'wali', 'parent', 'guardian', 'orang tua', 'orang tua / wali'
  )
  AND rp.permission IN (
      'Teacher.Read', 'Teacher.Create', 'Teacher.Update', 'Teacher.Delete',
      'Student.Read', 'Student.Create', 'Student.Update', 'Student.Delete',
      'Academic.Manage', 'School.Update',
      'Learning.Curriculum.Create', 'Learning.Curriculum.Update', 'Learning.Curriculum.Delete',
      'Learning.Material.Create', 'Learning.Material.Update', 'Learning.Material.Delete',
      'Learning.Lesson.Create', 'Learning.Lesson.Update', 'Learning.Lesson.Delete',
      'Learning.Session.Create', 'Learning.Session.Update',
      'Learning.Assignment.Create', 'Learning.Assignment.Update', 'Learning.Assignment.Delete',
      'Learning.Quiz.Create', 'Learning.Quiz.Update', 'Learning.Quiz.Delete', 'Learning.Quiz.Grade',
      'Learning.Assessment.Configure', 'Learning.Assessment.Grade',
      'Learning.Achievement.Create', 'Learning.Achievement.Award', 'Learning.Achievement.Delete'
  );

DELETE FROM role_permissions rp
USING roles r
WHERE rp.role_id = r.id
  AND (
      LOWER(r.name) LIKE '%guru%'
      OR LOWER(r.name) LIKE '%teacher%'
      OR LOWER(r.name) LIKE '%tutor%'
      OR LOWER(r.name) LIKE '%pamong%'
      OR LOWER(r.name) LIKE '%wali kelas%'
  )
  AND LOWER(r.name) NOT LIKE '%admin%'
  AND LOWER(r.name) NOT LIKE '%operator%'
  AND LOWER(r.name) NOT LIKE '%kepala%'
  AND rp.permission IN (
      'Academic.Manage',
      'School.Update',
      'Student.Create',
      'Student.Delete',
      'Teacher.Create',
      'Teacher.Update',
      'Teacher.Delete'
  );

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY,
    jti UUID NOT NULL UNIQUE,
    family_id UUID NOT NULL,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    parent_jti UUID,
    replaced_by_jti UUID,
    revoked_at TIMESTAMPTZ,
    revoke_reason VARCHAR(100),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_active
    ON refresh_tokens(tenant_id, user_id, expires_at)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_family
    ON refresh_tokens(family_id)
    WHERE revoked_at IS NULL;

ALTER TABLE refresh_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE refresh_tokens FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS refresh_tokens_tenant_isolation ON refresh_tokens;
CREATE POLICY refresh_tokens_tenant_isolation ON refresh_tokens
    FOR ALL
    USING (is_tenant_allowed(tenant_id))
    WITH CHECK (is_tenant_allowed(tenant_id));

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_runtime') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON refresh_tokens TO app_runtime;
    END IF;
EXCEPTION
    WHEN insufficient_privilege THEN
        RAISE NOTICE 'Skipping refresh_tokens grant for app_runtime';
END;
$$;
