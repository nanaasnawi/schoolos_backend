-- Migration: 20261014030000_standardize_four_attendance_statuses.sql
-- Description: Standardize attendance system to 4 statuses: present (H), sick (S), excused (I), absent (A).
--              Migrate any legacy 'late' statuses to 'present' and update database constraint.

-- 1. Convert any legacy 'late' status to 'present'
UPDATE session_attendances
SET status = 'present'
WHERE status = 'late';

-- 2. Drop existing status check constraint
ALTER TABLE session_attendances
    DROP CONSTRAINT IF EXISTS chk_session_attendances_status;

-- 3. Add updated constraint strictly allowing only the 4 standard statuses
ALTER TABLE session_attendances
    ADD CONSTRAINT chk_session_attendances_status
    CHECK (status IN ('present', 'absent', 'excused', 'sick'));
