-- Classroom Feed Domain
-- Phase 2.11

CREATE TABLE classroom_feeds (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    class_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    actor_name VARCHAR(255) NOT NULL DEFAULT '',
    action VARCHAR(100) NOT NULL,
    target_type VARCHAR(50),
    target_id UUID,
    summary TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_classroom_feeds_tenant ON classroom_feeds(tenant_id);
CREATE INDEX idx_classroom_feeds_class ON classroom_feeds(class_id);
CREATE INDEX idx_classroom_feeds_class_created ON classroom_feeds(class_id, created_at DESC);
