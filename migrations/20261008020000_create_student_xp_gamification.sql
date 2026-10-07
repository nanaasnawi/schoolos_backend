-- 20261008020000_create_student_xp_gamification.sql
-- Gamification & Student Experience Points (XP) Engine
-- Single Source of Truth for Reading Completions, Assignment Submissions, and Exam/Quiz completions

CREATE TABLE IF NOT EXISTS student_xp (
    student_id UUID PRIMARY KEY REFERENCES students(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    total_xp BIGINT NOT NULL DEFAULT 0,
    level INT NOT NULL DEFAULT 1,
    streak_days INT NOT NULL DEFAULT 1,
    last_activity_date DATE DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_student_xp_tenant ON student_xp(tenant_id);
CREATE INDEX IF NOT EXISTS idx_student_xp_total ON student_xp(total_xp DESC);

CREATE TABLE IF NOT EXISTS student_xp_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    action_type VARCHAR(50) NOT NULL, -- 'READ_MATERIAL', 'SUBMIT_ASSIGNMENT', 'COMPLETE_QUIZ', 'COMPLETE_EXAM'
    reference_id UUID,
    xp_amount INT NOT NULL DEFAULT 0,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_student_xp_activity UNIQUE(student_id, action_type, reference_id)
);

CREATE INDEX IF NOT EXISTS idx_student_xp_tx_student ON student_xp_transactions(student_id);
CREATE INDEX IF NOT EXISTS idx_student_xp_tx_tenant ON student_xp_transactions(tenant_id);
CREATE INDEX IF NOT EXISTS idx_student_xp_tx_created ON student_xp_transactions(created_at DESC);
