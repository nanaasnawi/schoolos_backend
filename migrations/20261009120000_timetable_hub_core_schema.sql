-- ============================================================================
-- Migration: 20261009120000_timetable_hub_core_schema.sql
-- Description: Timetable Hub core schema with end-to-end composite multi-tenant
--              isolation, defensive date backfill, strict TIME types, and RESTRICT.
-- ============================================================================

-- 1. PRE-REQUISITE: COMPOSITE UNIQUE KEYS ON PARENT TABLES (FOR STRICT MULTI-TENANT FKs)
ALTER TABLE classes
    DROP CONSTRAINT IF EXISTS uq_classes_tenant_id,
    ADD CONSTRAINT uq_classes_tenant_id UNIQUE (tenant_id, id);

ALTER TABLE teachers
    DROP CONSTRAINT IF EXISTS uq_teachers_tenant_id,
    ADD CONSTRAINT uq_teachers_tenant_id UNIQUE (tenant_id, id);

ALTER TABLE subjects
    DROP CONSTRAINT IF EXISTS uq_subjects_tenant_id,
    ADD CONSTRAINT uq_subjects_tenant_id UNIQUE (tenant_id, id);

ALTER TABLE students
    DROP CONSTRAINT IF EXISTS uq_students_tenant_id,
    ADD CONSTRAINT uq_students_tenant_id UNIQUE (tenant_id, id);

ALTER TABLE class_schedules
    DROP CONSTRAINT IF EXISTS uq_class_schedules_tenant_id,
    ADD CONSTRAINT uq_class_schedules_tenant_id UNIQUE (tenant_id, id);

ALTER TABLE quizzes
    DROP CONSTRAINT IF EXISTS uq_quizzes_tenant_id,
    ADD CONSTRAINT uq_quizzes_tenant_id UNIQUE (tenant_id, id);

-- 2. LEARNING SESSIONS ENHANCEMENT
ALTER TABLE learning_sessions 
    ALTER COLUMN lesson_id DROP NOT NULL;

ALTER TABLE learning_sessions
    ADD COLUMN IF NOT EXISTS session_type VARCHAR(20) NOT NULL DEFAULT 'scheduled',
    ADD COLUMN IF NOT EXISTS schedule_id UUID,
    ADD COLUMN IF NOT EXISTS subject_id UUID,
    ADD COLUMN IF NOT EXISTS session_number INT DEFAULT 1,
    ADD COLUMN IF NOT EXISTS start_time TIME,
    ADD COLUMN IF NOT EXISTS end_time TIME,
    ADD COLUMN IF NOT EXISTS substitute_teacher_id UUID,
    ADD COLUMN IF NOT EXISTS cancellation_reason TEXT;

-- Defensive Three-Step Backfill for session_date
ALTER TABLE learning_sessions 
    ADD COLUMN IF NOT EXISTS session_date DATE;

UPDATE learning_sessions 
SET session_date = COALESCE(scheduled_at::DATE, CURRENT_DATE) 
WHERE session_date IS NULL;

ALTER TABLE learning_sessions 
    ALTER COLUMN session_date SET NOT NULL;

-- Composite FKs: Ensuring schedule, subject, class, teacher, and substitute belong to the SAME tenant
ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_schedule,
    ADD CONSTRAINT fk_learning_sessions_schedule 
    FOREIGN KEY (tenant_id, schedule_id) 
    REFERENCES class_schedules(tenant_id, id) ON DELETE RESTRICT;

ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_subject,
    ADD CONSTRAINT fk_learning_sessions_subject 
    FOREIGN KEY (tenant_id, subject_id) 
    REFERENCES subjects(tenant_id, id) ON DELETE RESTRICT;

ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS fk_learning_sessions_substitute,
    ADD CONSTRAINT fk_learning_sessions_substitute 
    FOREIGN KEY (tenant_id, substitute_teacher_id) 
    REFERENCES teachers(tenant_id, id) ON DELETE SET NULL;

-- CHECK constraints for session validation & time ordering
ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_type,
    ADD CONSTRAINT chk_learning_sessions_type 
    CHECK (session_type IN ('scheduled', 'adhoc'));

ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_time,
    ADD CONSTRAINT chk_learning_sessions_time 
    CHECK (
        (start_time IS NULL AND end_time IS NULL) OR 
        (start_time IS NOT NULL AND end_time IS NOT NULL AND end_time > start_time)
    );

ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS chk_learning_sessions_context,
    ADD CONSTRAINT chk_learning_sessions_context 
    CHECK (
        (session_type = 'scheduled' AND schedule_id IS NOT NULL)
        OR
        (session_type = 'adhoc' AND schedule_id IS NULL AND subject_id IS NOT NULL AND start_time IS NOT NULL AND end_time IS NOT NULL)
    );

-- Unique composite constraint on learning_sessions parent (for session_attendances composite FK)
ALTER TABLE learning_sessions
    DROP CONSTRAINT IF EXISTS uq_learning_sessions_tenant_id,
    ADD CONSTRAINT uq_learning_sessions_tenant_id UNIQUE (tenant_id, id);

-- Anti-duplicate session partial unique index (deleted_at exists in base table)
CREATE UNIQUE INDEX IF NOT EXISTS uq_learning_sessions_schedule_date 
    ON learning_sessions(tenant_id, schedule_id, session_date) 
    WHERE schedule_id IS NOT NULL AND deleted_at IS NULL;

-- 3. ATTENDANCE PRESERVATION & COMPOSITE TENANT ISOLATION
-- Drop legacy individual FKs with CASCADE to strictly enforce RESTRICT policy
ALTER TABLE session_attendances 
    DROP CONSTRAINT IF EXISTS session_attendances_session_id_fkey,
    DROP CONSTRAINT IF EXISTS session_attendances_student_id_fkey,
    DROP CONSTRAINT IF EXISTS fk_session_attendances_session,
    DROP CONSTRAINT IF EXISTS fk_session_attendances_student;

ALTER TABLE session_attendances 
    ADD CONSTRAINT fk_session_attendances_session 
    FOREIGN KEY (tenant_id, session_id) 
    REFERENCES learning_sessions(tenant_id, id) ON DELETE RESTRICT;

ALTER TABLE session_attendances 
    ADD CONSTRAINT fk_session_attendances_student 
    FOREIGN KEY (tenant_id, student_id) 
    REFERENCES students(tenant_id, id) ON DELETE RESTRICT;

-- 4. CBT PROCTORED ENGINE & RATE LIMITING
ALTER TABLE quizzes
    ADD COLUMN IF NOT EXISTS session_id UUID,
    ADD COLUMN IF NOT EXISTS exam_mode VARCHAR(20) NOT NULL DEFAULT 'HOMEWORK_QUIZ',
    ADD COLUMN IF NOT EXISTS exam_token VARCHAR(10),
    ADD COLUMN IF NOT EXISTS token_expires_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS max_token_attempts INT NOT NULL DEFAULT 5;

-- Quiz session composite FK
ALTER TABLE quizzes
    DROP CONSTRAINT IF EXISTS fk_quizzes_session,
    ADD CONSTRAINT fk_quizzes_session 
    FOREIGN KEY (tenant_id, session_id) 
    REFERENCES learning_sessions(tenant_id, id) ON DELETE SET NULL;

ALTER TABLE quizzes
    DROP CONSTRAINT IF EXISTS chk_quizzes_exam_mode,
    ADD CONSTRAINT chk_quizzes_exam_mode 
    CHECK (exam_mode IN ('HOMEWORK_QUIZ', 'PROCTORED_CBT'));

ALTER TABLE quizzes
    DROP CONSTRAINT IF EXISTS chk_quizzes_token_attempts_limit,
    ADD CONSTRAINT chk_quizzes_token_attempts_limit 
    CHECK (max_token_attempts > 0 AND max_token_attempts <= 20);

-- Table for persistent, concurrency-safe token attempt tracking with composite multi-tenant FKs
CREATE TABLE IF NOT EXISTS quiz_token_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    quiz_id UUID NOT NULL,
    student_id UUID NOT NULL,
    failed_attempts INT NOT NULL DEFAULT 0,
    locked_until TIMESTAMP WITH TIME ZONE,
    last_attempt_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_quiz_token_attempts UNIQUE (tenant_id, quiz_id, student_id),
    CONSTRAINT fk_quiz_token_attempts_quiz
        FOREIGN KEY (tenant_id, quiz_id)
        REFERENCES quizzes(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_quiz_token_attempts_student
        FOREIGN KEY (tenant_id, student_id)
        REFERENCES students(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_quiz_token_attempts_tenant ON quiz_token_attempts(tenant_id);

-- 5. LEARNING MATERIALS & ASSIGNMENTS TIMING
ALTER TABLE learning_materials
    ADD COLUMN IF NOT EXISTS session_id UUID,
    ADD COLUMN IF NOT EXISTS release_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

ALTER TABLE learning_materials
    DROP CONSTRAINT IF EXISTS fk_learning_materials_session,
    ADD CONSTRAINT fk_learning_materials_session 
    FOREIGN KEY (tenant_id, session_id) 
    REFERENCES learning_sessions(tenant_id, id) ON DELETE SET NULL;

ALTER TABLE assignments
    ADD COLUMN IF NOT EXISTS session_id UUID,
    ADD COLUMN IF NOT EXISTS release_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS allow_late_submission BOOLEAN NOT NULL DEFAULT true;

ALTER TABLE assignments
    DROP CONSTRAINT IF EXISTS fk_assignments_session,
    ADD CONSTRAINT fk_assignments_session 
    FOREIGN KEY (tenant_id, session_id) 
    REFERENCES learning_sessions(tenant_id, id) ON DELETE SET NULL;
