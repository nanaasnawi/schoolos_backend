-- 20261008060000_unique_student_user_id.sql
-- Enforce 1:1 mapping between student profiles and user login accounts per tenant.
-- In Indonesia, students frequently share identical full names (e.g. ROHMAN, SRI ASTUTI, SITI).
-- This unique constraint guarantees that no two students can ever share or be linked to the same user account.

-- 1. Heal any existing mismatched user_id mappings by matching against the canonical student email (nisn@siswa.schoolos.id)
UPDATE students s
SET user_id = u.id, updated_at = NOW()
FROM users u
WHERE u.email = s.nisn || '@siswa.schoolos.id'
  AND u.tenant_id = s.tenant_id
  AND (s.user_id IS NULL OR s.user_id != u.id);

-- 2. Create unique index to strictly enforce 1 user account per student profile
CREATE UNIQUE INDEX IF NOT EXISTS idx_students_tenant_user_id 
ON students(tenant_id, user_id) 
WHERE user_id IS NOT NULL;
