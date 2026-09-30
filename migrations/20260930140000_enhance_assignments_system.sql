-- Migration: 20260930140000_enhance_assignments_system.sql
-- Description: Standardize assignment & submission statuses, add check constraints, and performance indexes across tenant isolation

-- 1. Normalize existing assignments data
UPDATE assignments 
SET status = LOWER(TRIM(status)) 
WHERE status IS NOT NULL;

UPDATE assignments 
SET status = 'published' 
WHERE status NOT IN ('draft', 'published', 'closed', 'archived');

-- 2. Add CHECK constraint on assignments(status)
ALTER TABLE assignments DROP CONSTRAINT IF EXISTS chk_assignments_status;
ALTER TABLE assignments ADD CONSTRAINT chk_assignments_status 
    CHECK (status IN ('draft', 'published', 'closed', 'archived'));

-- 3. Normalize existing assignment_submissions data
UPDATE assignment_submissions 
SET status = LOWER(TRIM(status)) 
WHERE status IS NOT NULL;

UPDATE assignment_submissions 
SET status = 'submitted' 
WHERE status NOT IN ('draft', 'submitted', 'grading', 'graded', 'returned', 'late');

-- 4. Add CHECK constraint on assignment_submissions(status)
ALTER TABLE assignment_submissions DROP CONSTRAINT IF EXISTS chk_assignment_submissions_status;
ALTER TABLE assignment_submissions ADD CONSTRAINT chk_assignment_submissions_status 
    CHECK (status IN ('draft', 'submitted', 'grading', 'graded', 'returned', 'late'));

-- 5. Add composite performance indexes for multi-tenant and class filtering
CREATE INDEX IF NOT EXISTS idx_assignments_tenant_class ON assignments(tenant_id, class_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assignments_tenant_status ON assignments(tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assignment_submissions_tenant_student ON assignment_submissions(tenant_id, student_id);
CREATE INDEX IF NOT EXISTS idx_assignment_submissions_assignment_student ON assignment_submissions(assignment_id, student_id);
