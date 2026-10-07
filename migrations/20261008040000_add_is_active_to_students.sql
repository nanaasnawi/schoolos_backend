-- Add is_active column to students for backward compatibility and fast boolean checks
ALTER TABLE students ADD COLUMN IF NOT EXISTS is_active BOOLEAN DEFAULT true;
UPDATE students SET is_active = (CASE WHEN status ILIKE 'active' THEN true ELSE false END);
CREATE INDEX IF NOT EXISTS idx_students_tenant_is_active ON students(tenant_id, is_active) WHERE deleted_at IS NULL;
