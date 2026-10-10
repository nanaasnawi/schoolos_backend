-- 20261010203000_create_kurikulum_merdeka_modul_ajar.sql
-- Kurikulum Merdeka: Modul Ajar (RPP) Bounded Context
-- 1. Tabel modul_ajar terikat ke learning_objectives (TP PUBLISHED)
-- 2. Strategi Berdiferensiasi 3-Pilar (Konten, Proses: VAK/Scaffolding, Produk)
-- 3. Trigger suspensi reaktif otomatis jika status TP induk diturunkan

CREATE TABLE IF NOT EXISTS modul_ajar (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    learning_objective_id UUID NOT NULL REFERENCES learning_objectives(id) ON DELETE RESTRICT,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL CHECK (semester IN ('ODD', 'EVEN')),
    title VARCHAR(255) NOT NULL,
    grade_level VARCHAR(50) NOT NULL,
    subject_code VARCHAR(50) NOT NULL,
    subject_name VARCHAR(100) NOT NULL,
    phase VARCHAR(20) NOT NULL,
    allocated_hours INTEGER NOT NULL DEFAULT 4 CHECK (allocated_hours > 0),
    total_meetings INTEGER NOT NULL DEFAULT 1 CHECK (total_meetings > 0),
    hours_per_meeting INTEGER NOT NULL DEFAULT 2 CHECK (hours_per_meeting > 0),
    pancasila_profiles TEXT[] DEFAULT ARRAY[]::TEXT[],
    meaningful_understanding TEXT NOT NULL,
    trigger_questions JSONB NOT NULL DEFAULT '[]'::jsonb,
    differentiation_strategies JSONB NOT NULL DEFAULT '{}'::jsonb,
    learning_activities JSONB NOT NULL DEFAULT '{}'::jsonb,
    assessment_plan JSONB NOT NULL DEFAULT '{}'::jsonb,
    lkpd_attachments JSONB NOT NULL DEFAULT '[]'::jsonb,
    status VARCHAR(30) NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT', 'ACTIVE', 'SUSPENDED', 'ARCHIVED')),
    suspension_reason TEXT DEFAULT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    is_ai_generated BOOLEAN NOT NULL DEFAULT false,
    ai_generation_meta JSONB DEFAULT NULL,
    created_by UUID,
    updated_by UUID,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

-- Indexing untuk query performa tinggi
CREATE INDEX IF NOT EXISTS idx_modul_ajar_tenant ON modul_ajar(tenant_id);
CREATE INDEX IF NOT EXISTS idx_modul_ajar_tp ON modul_ajar(learning_objective_id);
CREATE INDEX IF NOT EXISTS idx_modul_ajar_year_sem_sub ON modul_ajar(academic_year, semester, subject_code);
CREATE INDEX IF NOT EXISTS idx_modul_ajar_status ON modul_ajar(status);
CREATE INDEX IF NOT EXISTS idx_modul_ajar_deleted_at ON modul_ajar(deleted_at) WHERE deleted_at IS NULL;

-- Trigger updated_at otomatis
DROP TRIGGER IF EXISTS update_modul_ajar_updated_at ON modul_ajar;
CREATE TRIGGER update_modul_ajar_updated_at
    BEFORE UPDATE ON modul_ajar
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- TRIGGER SUSPENSI REAKTIF:
-- Menjaga integritas kurikulum. Jika TP diturunkan dari PUBLISHED ke DRAFT/REVIEWED/ARCHIVED,
-- seluruh Modul Ajar (RPP) yang bergantung pada TP tersebut otomatis berstatus SUSPENDED.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION cascade_suspend_modul_ajar_on_tp_demote()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.publication_status = 'PUBLISHED' AND NEW.publication_status != 'PUBLISHED' THEN
        UPDATE modul_ajar
        SET 
            status = 'SUSPENDED',
            suspension_reason = 'Tujuan Pembelajaran induk (' || NEW.code || ') diturunkan statusnya dari PUBLISHED menjadi ' || NEW.publication_status || ' pada ' || TO_CHAR(NOW(), 'YYYY-MM-DD HH24:MI:SS UTC'),
            updated_at = NOW()
        WHERE learning_objective_id = NEW.id
          AND status = 'ACTIVE'
          AND deleted_at IS NULL;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cascade_suspend_modul_ajar ON learning_objectives;
CREATE TRIGGER trg_cascade_suspend_modul_ajar
    AFTER UPDATE OF publication_status ON learning_objectives
    FOR EACH ROW
    EXECUTE FUNCTION cascade_suspend_modul_ajar_on_tp_demote();
