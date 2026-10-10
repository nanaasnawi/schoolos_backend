-- 20261010193000_enhance_kurikulum_merdeka_governance_and_provenance_down.sql
-- Rollback for governance and provenance enhancement

DROP TABLE IF EXISTS learning_flow_calendar_events CASCADE;
DROP TABLE IF EXISTS curriculum_ai_cache CASCADE;

ALTER TABLE learning_outcomes
    DROP COLUMN IF EXISTS source_origin,
    DROP COLUMN IF EXISTS source_version,
    DROP COLUMN IF EXISTS source_document,
    DROP COLUMN IF EXISTS document_page_ref,
    DROP COLUMN IF EXISTS verification_status,
    DROP COLUMN IF EXISTS verified_by,
    DROP COLUMN IF EXISTS verified_at,
    DROP COLUMN IF EXISTS provenance_audit_trail;

ALTER TABLE learning_objectives
    DROP COLUMN IF EXISTS publication_status,
    DROP COLUMN IF EXISTS version,
    DROP COLUMN IF EXISTS is_superseded,
    DROP COLUMN IF EXISTS ai_generation_meta,
    DROP COLUMN IF EXISTS reviewed_by,
    DROP COLUMN IF EXISTS reviewed_at,
    DROP COLUMN IF EXISTS published_by,
    DROP COLUMN IF EXISTS published_at;

ALTER TABLE learning_objective_flows
    DROP COLUMN IF EXISTS publication_status,
    DROP COLUMN IF EXISTS version,
    DROP COLUMN IF EXISTS is_superseded,
    DROP COLUMN IF EXISTS reviewed_by,
    DROP COLUMN IF EXISTS reviewed_at,
    DROP COLUMN IF EXISTS published_by,
    DROP COLUMN IF EXISTS published_at;
