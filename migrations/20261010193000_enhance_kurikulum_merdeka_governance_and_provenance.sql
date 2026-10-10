-- 20261010193000_enhance_kurikulum_merdeka_governance_and_provenance.sql
-- Kurikulum Merdeka Pedagogical Governance:
-- 1. Netralisasi default provenance & verifikasi eksplisit
-- 2. Lifecycle state machine terpadu (DRAFT, REVIEWED, PUBLISHED, ARCHIVED)
-- 3. Versioning & anti-overwrite draf AI (is_superseded, trace_id)
-- 4. Tabel relasi foreign-key ke Kalender Pendidikan (Kaldik)
-- 5. Tabel cache AI terisolasi dengan composite key dan status audit

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. LEARNING_OUTCOMES (CP) ENHANCEMENT
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE learning_outcomes
    ADD COLUMN IF NOT EXISTS source_origin VARCHAR(50) NOT NULL DEFAULT 'UNVERIFIED',
    ADD COLUMN IF NOT EXISTS source_version VARCHAR(50) NOT NULL DEFAULT 'UNSPECIFIED',
    ADD COLUMN IF NOT EXISTS source_document VARCHAR(255) DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS document_page_ref VARCHAR(100) DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS verification_status VARCHAR(30) NOT NULL DEFAULT 'UNVERIFIED_DRAFT',
    ADD COLUMN IF NOT EXISTS verified_by UUID,
    ADD COLUMN IF NOT EXISTS verified_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS provenance_audit_trail JSONB NOT NULL DEFAULT '[]'::jsonb;

-- Constraint check untuk status verifikasi dan asal sumber
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_lo_verification_status'
    ) THEN
        ALTER TABLE learning_outcomes
            ADD CONSTRAINT chk_lo_verification_status
            CHECK (verification_status IN ('NATIONAL_VERIFIED', 'SCHOOL_VERIFIED', 'UNVERIFIED_DRAFT'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_lo_source_origin'
    ) THEN
        ALTER TABLE learning_outcomes
            ADD CONSTRAINT chk_lo_source_origin
            CHECK (source_origin IN ('OFFICIAL_BSKAP', 'OFFICIAL_KEMENDIKDASMEN', 'SCHOOL_ADAPTED', 'AI_ASSISTED_DRAFT', 'UNVERIFIED'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_lo_verification_status ON learning_outcomes(verification_status);
CREATE INDEX IF NOT EXISTS idx_lo_source_origin ON learning_outcomes(source_origin);

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. LEARNING_OBJECTIVES (TP) LIFECYCLE & AUDIT
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE learning_objectives
    ADD COLUMN IF NOT EXISTS publication_status VARCHAR(30) NOT NULL DEFAULT 'DRAFT',
    ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS is_superseded BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS ai_generation_meta JSONB DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS reviewed_by UUID,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS published_by UUID,
    ADD COLUMN IF NOT EXISTS published_at TIMESTAMP WITH TIME ZONE;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_tp_publication_status'
    ) THEN
        ALTER TABLE learning_objectives
            ADD CONSTRAINT chk_tp_publication_status
            CHECK (publication_status IN ('DRAFT', 'REVIEWED', 'PUBLISHED', 'ARCHIVED'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_tp_tenant_status ON learning_objectives(tenant_id, publication_status);
CREATE INDEX IF NOT EXISTS idx_tp_version_superseded ON learning_objectives(learning_outcome_id, version, is_superseded);

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. LEARNING_OBJECTIVE_FLOWS (ATP) LIFECYCLE & AUDIT
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE learning_objective_flows
    ADD COLUMN IF NOT EXISTS publication_status VARCHAR(30) NOT NULL DEFAULT 'DRAFT',
    ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS is_superseded BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS reviewed_by UUID,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS published_by UUID,
    ADD COLUMN IF NOT EXISTS published_at TIMESTAMP WITH TIME ZONE;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_atp_publication_status'
    ) THEN
        ALTER TABLE learning_objective_flows
            ADD CONSTRAINT chk_atp_publication_status
            CHECK (publication_status IN ('DRAFT', 'REVIEWED', 'PUBLISHED', 'ARCHIVED'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_atp_tenant_status_year ON learning_objective_flows(tenant_id, academic_year, semester, publication_status);

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. JUNCTION TABLE: LEARNING_FLOW_CALENDAR_EVENTS (Relasi Ketat FK ke Kaldik)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS learning_flow_calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    flow_id UUID NOT NULL REFERENCES learning_objective_flows(id) ON DELETE CASCADE,
    calendar_event_id UUID NOT NULL REFERENCES academic_calendar_events(id) ON DELETE CASCADE,
    week_number INTEGER,
    allocated_hours INTEGER NOT NULL DEFAULT 2,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(flow_id, calendar_event_id)
);

CREATE INDEX IF NOT EXISTS idx_lfce_flow_id ON learning_flow_calendar_events(flow_id);
CREATE INDEX IF NOT EXISTS idx_lfce_event_id ON learning_flow_calendar_events(calendar_event_id);
CREATE INDEX IF NOT EXISTS idx_lfce_tenant_week ON learning_flow_calendar_events(tenant_id, week_number);

CREATE TRIGGER update_learning_flow_calendar_events_updated_at
    BEFORE UPDATE ON learning_flow_calendar_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- 5. CURRICULUM_AI_CACHE (Audit, Idempotency, and Concurrency Lock)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS curriculum_ai_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    cache_key VARCHAR(128) NOT NULL UNIQUE,
    source_cp_id UUID REFERENCES learning_outcomes(id) ON DELETE CASCADE,
    source_cp_version VARCHAR(50) NOT NULL DEFAULT 'UNSPECIFIED',
    subject_code VARCHAR(50),
    phase VARCHAR(20) NOT NULL,
    grade_level VARCHAR(50) NOT NULL,
    academic_year VARCHAR(50) NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    prompt_version VARCHAR(50) NOT NULL,
    status VARCHAR(30) NOT NULL DEFAULT 'DRAFT_GENERATED',
    raw_response JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE
);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_cache_status'
    ) THEN
        ALTER TABLE curriculum_ai_cache
            ADD CONSTRAINT chk_cache_status
            CHECK (status IN ('DRAFT_GENERATED', 'APPLIED_TO_DRAFT', 'SUPERSEDED', 'FAILED'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_ai_cache_key ON curriculum_ai_cache(cache_key);
CREATE INDEX IF NOT EXISTS idx_ai_cache_lookup ON curriculum_ai_cache(tenant_id, phase, subject_code);

-- ─────────────────────────────────────────────────────────────────────────────
-- 6. NORMALISASI DATA BASELINE LAMA (Pembersihan Status Otomatis)
-- ─────────────────────────────────────────────────────────────────────────────
-- Pastikan baris seed awal tidak mengklaim verifikasi tanpa bukti audit
UPDATE learning_outcomes
SET source_origin = 'UNVERIFIED',
    verification_status = 'UNVERIFIED_DRAFT',
    source_version = 'DRAFT-INIT',
    provenance_audit_trail = jsonb_build_array(
        jsonb_build_object(
            'action', 'INITIAL_IMPORT',
            'timestamp', NOW(),
            'notes', 'Data template awal, menunggu pencocokan naskah resmi Kemendikdasmen RI'
        )
    )
WHERE verification_status IS NULL OR verification_status = 'UNVERIFIED_DRAFT';

UPDATE learning_objectives
SET publication_status = 'DRAFT'
WHERE publication_status IS NULL;

UPDATE learning_objective_flows
SET publication_status = 'DRAFT'
WHERE publication_status IS NULL;
