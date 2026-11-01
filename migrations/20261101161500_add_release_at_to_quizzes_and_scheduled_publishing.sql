-- 20261101161500_add_release_at_to_quizzes_and_scheduled_publishing.sql
-- Description: Add release_at to quizzes, and notification_dispatched tracking for scheduled publications (ADR-0011, ADR-0013)

-- 1. Add release_at and notification_dispatched to quizzes
ALTER TABLE quizzes
    ADD COLUMN IF NOT EXISTS release_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN NOT NULL DEFAULT false;

-- 2. Add notification_dispatched to learning_materials
ALTER TABLE learning_materials
    ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN NOT NULL DEFAULT false;

-- 3. Add notification_dispatched to assignments
ALTER TABLE assignments
    ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN NOT NULL DEFAULT false;

-- 4. Set existing published/past materials, assignments, and quizzes as already dispatched so worker doesn't spam old records
UPDATE learning_materials 
SET notification_dispatched = true 
WHERE release_at IS NULL OR release_at <= NOW();

UPDATE assignments 
SET notification_dispatched = true 
WHERE release_at IS NULL OR release_at <= NOW();

UPDATE quizzes 
SET notification_dispatched = true 
WHERE release_at IS NULL OR release_at <= NOW();

-- 5. Performance Indices for Scheduled Worker & Read Queries
CREATE INDEX IF NOT EXISTS idx_quizzes_release_at ON quizzes(release_at);
CREATE INDEX IF NOT EXISTS idx_learning_materials_release_at ON learning_materials(release_at);
CREATE INDEX IF NOT EXISTS idx_assignments_release_at ON assignments(release_at);

CREATE INDEX IF NOT EXISTS idx_learning_materials_publish_worker 
    ON learning_materials(tenant_id, release_at, notification_dispatched) 
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_assignments_publish_worker 
    ON assignments(tenant_id, release_at, notification_dispatched) 
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_quizzes_publish_worker 
    ON quizzes(tenant_id, release_at, notification_dispatched) 
    WHERE deleted_at IS NULL;

