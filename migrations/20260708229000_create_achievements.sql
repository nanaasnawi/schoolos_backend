-- Achievement & Gamification Domain
-- Phase 2.10

CREATE TABLE achievements (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    icon VARCHAR(100) NOT NULL DEFAULT '🏆',
    criteria_type VARCHAR(50) NOT NULL,
    criteria_value TEXT NOT NULL,
    is_published BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    deleted_by UUID,
    version INT NOT NULL DEFAULT 1
);

CREATE TABLE student_achievements (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    student_id UUID NOT NULL,
    achievement_id UUID NOT NULL REFERENCES achievements(id),
    earned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_achievements_tenant ON achievements(tenant_id);
CREATE INDEX idx_achievements_deleted ON achievements(deleted_at);
CREATE INDEX idx_student_achievements_student ON student_achievements(student_id);
CREATE INDEX idx_student_achievements_achievement ON student_achievements(achievement_id);
CREATE INDEX idx_student_achievements_tenant ON student_achievements(tenant_id);
