-- Add persistent 1x thumbnail caching columns for learning materials and assignments.
-- Thumbnails are generated/resolved ONCE at publish time (never per student GET request)
-- and cached permanently via thumbnail_url + thumbnail_status.

ALTER TABLE learning_materials
    ADD COLUMN IF NOT EXISTS thumbnail_url TEXT,
    ADD COLUMN IF NOT EXISTS thumbnail_status VARCHAR(24) NOT NULL DEFAULT 'pending';

ALTER TABLE assignments
    ADD COLUMN IF NOT EXISTS thumbnail_url TEXT,
    ADD COLUMN IF NOT EXISTS thumbnail_status VARCHAR(24) NOT NULL DEFAULT 'pending';

CREATE INDEX IF NOT EXISTS idx_learning_materials_thumb_pending
    ON learning_materials (tenant_id, created_at DESC)
    WHERE deleted_at IS NULL AND thumbnail_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_assignments_thumb_pending
    ON assignments (tenant_id, created_at DESC)
    WHERE deleted_at IS NULL AND thumbnail_status = 'pending';
