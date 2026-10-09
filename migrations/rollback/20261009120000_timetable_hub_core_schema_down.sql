-- ============================================================================
-- Rollback: 20261009120000_timetable_hub_core_schema_down.sql
-- Description: Revert all DDL mutations prior to production traffic.
-- Note: Execute ONLY pre-deployment if migration fails.
--       Once live data is written, use forward-fix migrations instead.
-- ============================================================================

-- 1. Materials & Assignments
ALTER TABLE assignments 
    DROP CONSTRAINT IF EXISTS fk_assignments_session,
    DROP COLUMN IF EXISTS allow_late_submission, 
    DROP COLUMN IF EXISTS release_at, 
    DROP COLUMN IF EXISTS session_id;

ALTER TABLE learning_materials 
    DROP CONSTRAINT IF EXISTS fk_learning_materials_session,
    DROP COLUMN IF EXISTS release_at, 
    DROP COLUMN IF EXISTS session_id;

-- 2. CBT & Token Attempts Table
DROP TABLE IF EXISTS quiz_token_attempts CASCADE;

ALTER TABLE quizzes 
    DROP CONSTRAINT IF EXISTS fk_quizzes_session,
    DROP CONSTRAINT IF EXISTS chk_quizzes_token_attempts_limit,
    DROP CONSTRAINT IF EXISTS chk_quizzes_exam_mode,
    DROP COLUMN IF EXISTS max_token_attempts,
    DROP COLUMN IF EXISTS token_expires_at,
    DROP COLUMN IF EXISTS exam_token,
    DROP COLUMN IF EXISTS exam_mode,
    DROP COLUMN IF EXISTS session_id;

-- 3. Restore session_attendances state prior to this migration
ALTER TABLE session_attendances 
    DROP CONSTRAINT IF EXISTS fk_session_attendances_session,
    DROP CONSTRAINT IF EXISTS fk_session_attendances_student;

-- Re-establish original individual CASCADE foreign keys
ALTER TABLE session_attendances
    DROP CONSTRAINT IF EXISTS session_attendances_session_id_fkey,
    ADD CONSTRAINT session_attendances_session_id_fkey 
    FOREIGN KEY (session_id) REFERENCES learning_sessions(id) ON DELETE CASCADE;

ALTER TABLE session_attendances
    DROP CONSTRAINT IF EXISTS session_attendances_student_id_fkey,
    ADD CONSTRAINT session_attendances_student_id_fkey 
    FOREIGN KEY (student_id) REFERENCES students(id) ON DELETE CASCADE;

-- 4. Learning Sessions
DROP INDEX IF EXISTS uq_learning_sessions_schedule_date;

ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS uq_learning_sessions_tenant_id,
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_context,
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_time,
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_type,
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_substitute,
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_subject,
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_schedule,
    DROP COLUMN IF EXISTS cancellation_reason,
    DROP COLUMN IF EXISTS substitute_teacher_id,
    DROP COLUMN IF EXISTS end_time,
    DROP COLUMN IF EXISTS start_time,
    DROP COLUMN IF EXISTS session_number,
    DROP COLUMN IF EXISTS session_date,
    DROP COLUMN IF EXISTS subject_id,
    DROP COLUMN IF EXISTS schedule_id,
    DROP COLUMN IF EXISTS session_type;

-- Restore lesson_id NOT NULL if table remains clean
ALTER TABLE learning_sessions 
    ALTER COLUMN lesson_id SET NOT NULL;

-- 5. Drop Parent Composite Unique Keys
ALTER TABLE quizzes DROP CONSTRAINT IF EXISTS uq_quizzes_tenant_id;
ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS uq_class_schedules_tenant_id;
ALTER TABLE students DROP CONSTRAINT IF EXISTS uq_students_tenant_id;
ALTER TABLE subjects DROP CONSTRAINT IF EXISTS uq_subjects_tenant_id;
ALTER TABLE teachers DROP CONSTRAINT IF EXISTS uq_teachers_tenant_id;
ALTER TABLE classes DROP CONSTRAINT IF EXISTS uq_classes_tenant_id;
