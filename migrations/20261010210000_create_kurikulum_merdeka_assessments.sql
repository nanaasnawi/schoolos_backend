-- 20261010210000_create_kurikulum_merdeka_assessments.sql
-- Kurikulum Merdeka Pedagogical Ecosystem: Fase 4 Asesmen Berkelanjutan
-- 1. Taksonomi 3-Tingkat (Diagnostik, Formatif, Sumatif Lingkup Materi / SAS)
-- 2. Kriteria Ketercapaian Tujuan Pembelajaran (KKTP) menggantikan KKM
-- 3. Jembatan Diagnostik ke Diferensiasi & Agregasi e-Rapor Otomatis

CREATE TABLE IF NOT EXISTS pedagogical_assessments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    learning_objective_id UUID REFERENCES learning_objectives(id) ON DELETE RESTRICT,
    modul_ajar_id UUID REFERENCES modul_ajar(id) ON DELETE SET NULL,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL CHECK (semester IN ('ODD', 'EVEN')),
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    subject_code VARCHAR(50) NOT NULL,
    subject_name VARCHAR(100) NOT NULL,
    title VARCHAR(255) NOT NULL,
    taxonomy_type VARCHAR(30) NOT NULL CHECK (
        taxonomy_type IN (
            'DIAGNOSTIC_NON_COGNITIVE',
            'DIAGNOSTIC_COGNITIVE',
            'FORMATIVE',
            'SUMMATIVE_MATERIAL',
            'SUMMATIVE_SEMESTER'
        )
    ),
    assessment_method VARCHAR(50) NOT NULL DEFAULT 'WRITTEN_TEST' CHECK (
        assessment_method IN (
            'OBSERVATION',
            'PERFORMANCE',
            'PRODUCT',
            'PORTFOLIO',
            'WRITTEN_TEST',
            'SELF_PEER_ASSESSMENT',
            'QUESTIONNAIRE'
        )
    ),
    passing_threshold NUMERIC(5,2) NOT NULL DEFAULT 75.0,
    kktp_criteria JSONB NOT NULL DEFAULT '{}'::jsonb,
    rubric_descriptors JSONB NOT NULL DEFAULT '[]'::jsonb,
    date_conducted DATE NOT NULL DEFAULT CURRENT_DATE,
    calendar_event_id UUID REFERENCES academic_calendar_events(id) ON DELETE SET NULL,
    status VARCHAR(30) NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT', 'PUBLISHED', 'COMPLETED', 'ARCHIVED')),
    created_by UUID,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

-- Indeks performa pencarian asesmen
CREATE INDEX IF NOT EXISTS idx_ped_assess_tenant ON pedagogical_assessments(tenant_id);
CREATE INDEX IF NOT EXISTS idx_ped_assess_class ON pedagogical_assessments(class_id);
CREATE INDEX IF NOT EXISTS idx_ped_assess_tp ON pedagogical_assessments(learning_objective_id);
CREATE INDEX IF NOT EXISTS idx_ped_assess_taxonomy ON pedagogical_assessments(taxonomy_type);
CREATE INDEX IF NOT EXISTS idx_ped_assess_year_sem_sub ON pedagogical_assessments(academic_year, semester, subject_code);
CREATE INDEX IF NOT EXISTS idx_ped_assess_deleted_at ON pedagogical_assessments(deleted_at) WHERE deleted_at IS NULL;

-- Trigger updated_at
DROP TRIGGER IF EXISTS update_pedagogical_assessments_updated_at ON pedagogical_assessments;
CREATE TRIGGER update_pedagogical_assessments_updated_at
    BEFORE UPDATE ON pedagogical_assessments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- HASIL / NILAI ASESMEN SISWA (DIAGNOSTIK, FORMATIF, SUMATIF)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS assessment_student_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    assessment_id UUID NOT NULL REFERENCES pedagogical_assessments(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    raw_score NUMERIC(5,2) DEFAULT NULL,
    qualitative_level VARCHAR(50) DEFAULT NULL,
    feedback_notes TEXT DEFAULT NULL,
    strengths TEXT DEFAULT NULL,
    areas_for_improvement TEXT DEFAULT NULL,
    evidence_submission JSONB DEFAULT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(assessment_id, student_id)
);

CREATE INDEX IF NOT EXISTS idx_asr_tenant ON assessment_student_results(tenant_id);
CREATE INDEX IF NOT EXISTS idx_asr_assessment ON assessment_student_results(assessment_id);
CREATE INDEX IF NOT EXISTS idx_asr_student ON assessment_student_results(student_id);

DROP TRIGGER IF EXISTS update_assessment_student_results_updated_at ON assessment_student_results;
CREATE TRIGGER update_assessment_student_results_updated_at
    BEFORE UPDATE ON assessment_student_results
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
