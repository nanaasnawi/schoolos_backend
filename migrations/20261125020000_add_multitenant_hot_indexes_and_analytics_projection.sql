-- Consolidate tables previously created via runtime DDL, add strict RLS + FORCE RLS on them,
-- add multi-tenant composite hot indexes, and introduce analytics_school_overview projection table.

-- 1. Pre-start CBT notice deduplication tables
CREATE TABLE IF NOT EXISTS quiz_pre_start_notices (
    quiz_id UUID PRIMARY KEY REFERENCES quizzes(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    start_bucket VARCHAR(32) NOT NULL,
    notified_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS quiz_executive_pre_start_batches (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    start_bucket VARCHAR(32) NOT NULL,
    sent_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, user_id, start_bucket)
);

-- 2. Grade sync & proctoring schema columns/tables (moved out of runtime handlers)
ALTER TABLE assignments ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';
ALTER TABLE pedagogical_assessments ADD COLUMN IF NOT EXISTS source_type VARCHAR(32) DEFAULT NULL;
ALTER TABLE pedagogical_assessments ADD COLUMN IF NOT EXISTS source_id UUID DEFAULT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_ped_assess_source_unique
    ON pedagogical_assessments(tenant_id, source_type, source_id)
    WHERE source_id IS NOT NULL AND deleted_at IS NULL;
ALTER TABLE gradebook_snapshots ADD COLUMN IF NOT EXISTS ats_score NUMERIC(5,2) DEFAULT NULL;
ALTER TABLE assignment_questions ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL;
ALTER TABLE quiz_questions ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL;
ALTER TABLE attempt_answers ADD COLUMN IF NOT EXISTS teacher_feedback TEXT DEFAULT NULL;
ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN DEFAULT FALSE;

ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS leave_violations_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_violation_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_device_platform VARCHAR(32);

CREATE TABLE IF NOT EXISTS quiz_proctor_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    quiz_id UUID NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
    attempt_id UUID NOT NULL REFERENCES quiz_attempts(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    event_type VARCHAR(40) NOT NULL,
    detail TEXT,
    device_platform VARCHAR(32),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_quiz_proctor_events_quiz
    ON quiz_proctor_events(tenant_id, quiz_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_quiz_proctor_events_attempt
    ON quiz_proctor_events(attempt_id, created_at DESC);

-- 3. Analytics projection table (avoids repeated 16x COUNT(*) scans per dashboard view)
CREATE TABLE IF NOT EXISTS analytics_school_overview (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    total_students BIGINT NOT NULL DEFAULT 0,
    active_students BIGINT NOT NULL DEFAULT 0,
    transferred_students BIGINT NOT NULL DEFAULT 0,
    total_teachers BIGINT NOT NULL DEFAULT 0,
    active_teachers BIGINT NOT NULL DEFAULT 0,
    total_staff BIGINT NOT NULL DEFAULT 0,
    total_classes BIGINT NOT NULL DEFAULT 0,
    total_guardians BIGINT NOT NULL DEFAULT 0,
    total_materials BIGINT NOT NULL DEFAULT 0,
    total_assignments BIGINT NOT NULL DEFAULT 0,
    total_quizzes BIGINT NOT NULL DEFAULT 0,
    total_submissions BIGINT NOT NULL DEFAULT 0,
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 4. Apply strict tenant isolation RLS + FORCE RLS to newly tracked tenant tables
DO $$
DECLARE
    tbl TEXT;
BEGIN
    FOR tbl IN SELECT unnest(ARRAY[
        'quiz_pre_start_notices',
        'quiz_executive_pre_start_batches',
        'quiz_proctor_events',
        'analytics_school_overview',
        'ai_feedback_logs',
        'daily_quests',
        'daily_quest_attempts',
        'student_shiba_pets',
        'daily_quest_chest_claims',
        'daily_quest_bonus_logs'
    ]) LOOP
        IF to_regclass('public.' || tbl) IS NOT NULL THEN
            EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', tbl);
            EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', tbl);
            EXECUTE format('DROP POLICY IF EXISTS %I_tenant_isolation ON %I', tbl, tbl);
            EXECUTE format(
                'CREATE POLICY %I_tenant_isolation ON %I USING (is_tenant_allowed(tenant_id)) WITH CHECK (is_tenant_allowed(tenant_id))',
                tbl, tbl
            );
        END IF;
    END LOOP;
END $$;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_runtime') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO app_runtime;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO app_runtime;
    END IF;
END $$;

-- 5. Multi-tenant composite hot indexes for school-scale query latency
CREATE INDEX IF NOT EXISTS idx_students_tenant_active_deleted
    ON students(tenant_id, deleted_at, is_active);

CREATE INDEX IF NOT EXISTS idx_teachers_tenant_user_deleted
    ON teachers(tenant_id, user_id, deleted_at);

CREATE INDEX IF NOT EXISTS idx_guardians_tenant_user
    ON guardians(tenant_id, user_id);

CREATE INDEX IF NOT EXISTS idx_enrollments_tenant_class_status
    ON enrollments(tenant_id, class_id, status);

CREATE INDEX IF NOT EXISTS idx_enrollments_tenant_student_status
    ON enrollments(tenant_id, student_id, status);

CREATE INDEX IF NOT EXISTS idx_quiz_attempts_tenant_quiz_student
    ON quiz_attempts(tenant_id, quiz_id, student_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_assignments_tenant_class_deleted
    ON assignments(tenant_id, class_id, deleted_at, status);

CREATE INDEX IF NOT EXISTS idx_assignment_submissions_tenant_asg_student
    ON assignment_submissions(tenant_id, assignment_id, student_id);

CREATE INDEX IF NOT EXISTS idx_learning_materials_tenant_class_deleted
    ON learning_materials(tenant_id, class_id, deleted_at);

CREATE INDEX IF NOT EXISTS idx_gradebook_entries_tenant_class_subj_student
    ON gradebook_entries(tenant_id, class_id, subject_id, student_id);

CREATE INDEX IF NOT EXISTS idx_gradebook_snapshots_tenant_yr_sem
    ON gradebook_snapshots(tenant_id, academic_year, semester, student_id);

CREATE INDEX IF NOT EXISTS idx_notifications_tenant_user_read_created
    ON notifications(tenant_id, user_id, is_read, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_tenant_ts_desc
    ON audit_logs(tenant_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_created_at_asc
    ON idempotency_keys(created_at ASC);
