-- Migration: 20260926000000_relax_session_attendance_fkey.sql
-- Description: Drop foreign key constraint on session_attendances(session_id) to allow attendance tracking on dynamic class schedules as well as learning sessions.

ALTER TABLE session_attendances DROP CONSTRAINT IF EXISTS session_attendances_session_id_fkey;
