-- Migration: 20260930120000_enhance_attendance_system.sql
-- Description: Enhance session_attendances with performance indexes, methods, recorded_by, and status constraints.

-- 1. Add method and recorded_by columns if they do not exist
ALTER TABLE session_attendances 
    ADD COLUMN IF NOT EXISTS method VARCHAR(20) DEFAULT 'manual',
    ADD COLUMN IF NOT EXISTS recorded_by UUID;

-- 2. Add composite indexes for high-speed multi-tenant queries
CREATE INDEX IF NOT EXISTS idx_session_attendances_tenant_id 
    ON session_attendances(tenant_id);

CREATE INDEX IF NOT EXISTS idx_session_attendances_tenant_session 
    ON session_attendances(tenant_id, session_id);

CREATE INDEX IF NOT EXISTS idx_session_attendances_tenant_student 
    ON session_attendances(tenant_id, student_id);

CREATE INDEX IF NOT EXISTS idx_session_attendances_checked_in_at 
    ON session_attendances(checked_in_at);

-- 3. Normalize existing legacy statuses to standard lowercase
UPDATE session_attendances
SET status = CASE 
    WHEN LOWER(status) IN ('present', 'hadir', 'h') THEN 'present'
    WHEN LOWER(status) IN ('sick', 'sakit', 's') THEN 'sick'
    WHEN LOWER(status) IN ('excused', 'izin', 'i') THEN 'excused'
    WHEN LOWER(status) IN ('late', 'terlambat', 't') THEN 'late'
    ELSE 'absent'
END
WHERE status IS NOT NULL;

-- 4. Add check constraint on status
ALTER TABLE session_attendances
    DROP CONSTRAINT IF EXISTS chk_session_attendances_status;

ALTER TABLE session_attendances
    ADD CONSTRAINT chk_session_attendances_status
    CHECK (status IN ('present', 'absent', 'late', 'excused', 'sick'));
