-- Proctoring telemetry, heartbeat tracking, and violation log for CBT exams (ADR-0011, ADR-0013)

ALTER TABLE quiz_attempts
    ADD COLUMN IF NOT EXISTS leave_violations_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_violation_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS last_device_platform VARCHAR(32);

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
