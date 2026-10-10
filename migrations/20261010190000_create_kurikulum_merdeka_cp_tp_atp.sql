-- 20261010190000_create_kurikulum_merdeka_cp_tp_atp.sql
-- Kurikulum Merdeka Pedagogical Ecosystem:
-- 1. Capaian Pembelajaran (CP) per Fase & Elemen
-- 2. Tujuan Pembelajaran (TP) dengan Dekonstruksi Taksonomi Bloom & Profil Pelajar Pancasila
-- 3. Alur Tujuan Pembelajaran (ATP) terdistribusi per Semester & Pekan Efektif (Kaldik)

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. LEARNING OUTCOMES (CAPAIAN PEMBELAJARAN / CP)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS learning_outcomes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    subject_code VARCHAR(50),
    subject_name VARCHAR(150) NOT NULL,
    phase VARCHAR(20) NOT NULL, -- 'FASE_A', 'FASE_B', 'FASE_C', 'FASE_D', 'FASE_E', 'FASE_F'
    target_grades VARCHAR(50) NOT NULL, -- e.g. 'Kelas 5-6 SD', 'Kelas 7-9 SMP'
    element_name VARCHAR(150) NOT NULL, -- e.g. 'Pemahaman IPAS', 'Menyimak', 'Bilangan'
    element_code VARCHAR(50), -- e.g. 'ELEMEN_PEMAHAMAN', 'ELEMEN_MENYIMAK'
    description TEXT NOT NULL, -- Teks resmi Capaian Pembelajaran per elemen
    curriculum_standard VARCHAR(50) NOT NULL DEFAULT 'KURIKULUM_MERDEKA',
    regulation_reference VARCHAR(150) DEFAULT 'Kepmendikbudristek No. 032/H/KR/2024',
    is_ai_generated BOOLEAN NOT NULL DEFAULT false,
    is_verified BOOLEAN NOT NULL DEFAULT true,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX IF NOT EXISTS idx_lo_tenant_phase_subject ON learning_outcomes(tenant_id, phase, subject_name);
CREATE INDEX IF NOT EXISTS idx_lo_phase ON learning_outcomes(phase);
CREATE INDEX IF NOT EXISTS idx_lo_subject_id ON learning_outcomes(subject_id);
CREATE INDEX IF NOT EXISTS idx_lo_deleted_at ON learning_outcomes(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_learning_outcomes_updated_at
    BEFORE UPDATE ON learning_outcomes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. LEARNING OBJECTIVES (TUJUAN PEMBELAJARAN / TP)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS learning_objectives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    learning_outcome_id UUID NOT NULL REFERENCES learning_outcomes(id) ON DELETE CASCADE,
    code VARCHAR(50) NOT NULL, -- e.g. 'TP.IPAS.5.1', 'TP-01'
    competency VARCHAR(150), -- Kata Kerja Operasional (KKO), e.g. 'Menganalisis', 'Menjelaskan'
    bloom_level VARCHAR(20) DEFAULT 'C3', -- 'C1'..'C6'
    content_scope TEXT NOT NULL, -- Lingkup Materi / Konten Esensial
    statement TEXT NOT NULL, -- Rumusan Lengkap TP
    pancasila_profiles TEXT[] DEFAULT ARRAY[]::TEXT[], -- e.g. ARRAY['Bernalar Kritis', 'Mandiri']
    evidence_indicators TEXT[] DEFAULT ARRAY[]::TEXT[], -- IKTP / Eviden Ketercapaian
    estimated_hours INTEGER NOT NULL DEFAULT 4, -- Estimasi Jam Pelajaran (JP)
    order_index INTEGER NOT NULL DEFAULT 0,
    is_ai_generated BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX IF NOT EXISTS idx_tp_tenant_lo ON learning_objectives(tenant_id, learning_outcome_id);
CREATE INDEX IF NOT EXISTS idx_tp_code ON learning_objectives(code);
CREATE INDEX IF NOT EXISTS idx_tp_deleted_at ON learning_objectives(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_learning_objectives_updated_at
    BEFORE UPDATE ON learning_objectives
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. LEARNING OBJECTIVE FLOWS (ALUR TUJUAN PEMBELAJARAN / ATP)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS learning_objective_flows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    learning_objective_id UUID NOT NULL REFERENCES learning_objectives(id) ON DELETE CASCADE,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    grade_level VARCHAR(50) NOT NULL, -- e.g. 'Kelas 5 SD'
    semester VARCHAR(10) NOT NULL CHECK (semester IN ('ODD', 'EVEN')),
    sequence_order INTEGER NOT NULL DEFAULT 1,
    allocated_hours INTEGER NOT NULL DEFAULT 4,
    target_week_start INTEGER, -- Pekan efektif ke- berapa di Kaldik
    target_week_end INTEGER,
    pedagogical_approach VARCHAR(100) DEFAULT 'Problem-Based Learning',
    assessment_plan VARCHAR(150) DEFAULT 'Formatif (Observasi & Kuis) + Sumatif Lingkup Materi',
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX IF NOT EXISTS idx_atp_tenant_year_sem ON learning_objective_flows(tenant_id, academic_year, semester, grade_level);
CREATE INDEX IF NOT EXISTS idx_atp_lo_id ON learning_objective_flows(learning_objective_id);
CREATE INDEX IF NOT EXISTS idx_atp_deleted_at ON learning_objective_flows(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_learning_objective_flows_updated_at
    BEFORE UPDATE ON learning_objective_flows
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. SEED NATIONAL BASELINE TEMPLATES (Fase C - Kelas 5 SD)
-- ─────────────────────────────────────────────────────────────────────────────
-- 4.1 IPAS FASE C
WITH inserted_cp_ipas AS (
    INSERT INTO learning_outcomes (
        id, tenant_id, subject_code, subject_name, phase, target_grades, 
        element_name, element_code, description, is_ai_generated, is_verified, order_index
    ) VALUES (
        'c0000001-0000-0000-0000-000000000001',
        NULL,
        '401900000',
        'Ilmu Pengetahuan Alam dan Sosial (IPAS)',
        'FASE_C',
        'Kelas 5-6 SD',
        'Pemahaman IPAS (Sains dan Sosial)',
        'ELEMEN_PEMAHAMAN',
        'Peserta didik memahami sistem organ tubuh manusia (pernapasan, pencernaan, peredaran darah) yang dikaitkan dengan cara menjaga kesehatan tubuhnya; hubungan antar komponen biotik dan abiotik serta pengaruhnya terhadap ekosistem; fenomena gelombang bunyi dan cahaya dalam kehidupan sehari-hari; siklus air dan upaya menjaga ketersediaan air bersih; serta pemanfaatan energi alternatif ramah lingkungan.',
        false,
        true,
        1
    ) ON CONFLICT (id) DO NOTHING
    RETURNING id
),
inserted_tp_ipas_1 AS (
    INSERT INTO learning_objectives (
        id, tenant_id, learning_outcome_id, code, competency, bloom_level,
        content_scope, statement, pancasila_profiles, evidence_indicators, estimated_hours, order_index
    ) VALUES (
        'c0000002-0000-0000-0000-000000000001',
        NULL,
        'c0000001-0000-0000-0000-000000000001',
        'TP.IPAS.5.1',
        'Menganalisis',
        'C4',
        'Sistem Organ Pencernaan Manusia',
        'Peserta didik dapat menganalisis struktur dan fungsi organ pencernaan manusia serta merancang pola makan sehat bergizi seimbang untuk menjaga kesehatannya.',
        ARRAY['Bernalar Kritis', 'Mandiri'],
        ARRAY['Mengidentifikasi urutan organ pencernaan makanan', 'Menjelaskan fungsi enzim pencernaan utama', 'Menyajikan infografis menu makanan bergizi seimbang'],
        6,
        1
    ) ON CONFLICT (id) DO NOTHING
    RETURNING id
),
inserted_tp_ipas_2 AS (
    INSERT INTO learning_objectives (
        id, tenant_id, learning_outcome_id, code, competency, bloom_level,
        content_scope, statement, pancasila_profiles, evidence_indicators, estimated_hours, order_index
    ) VALUES (
        'c0000002-0000-0000-0000-000000000002',
        NULL,
        'c0000001-0000-0000-0000-000000000001',
        'TP.IPAS.5.2',
        'Menyelidiki',
        'C4',
        'Hubungan Biotik, Abiotik & Jaring-jaring Makanan',
        'Peserta didik dapat menyelidiki interaksi antara komponen biotik dan abiotik serta menganalisis dampak kepunahan salah satu populasi dalam jaring-jaring makanan ekosistem sekitar.',
        ARRAY['Bernalar Kritis', 'Gotong Royong'],
        ARRAY['Mengelompokkan produsen, konsumen, dan dekomposer', 'Membuat bagan jaring-jaring makanan ekosistem sawah/hutan', 'Merumuskan solusi pelestarian ekosistem lokal'],
        6,
        2
    ) ON CONFLICT (id) DO NOTHING
    RETURNING id
)
INSERT INTO learning_objective_flows (
    tenant_id, learning_objective_id, academic_year, grade_level, semester,
    sequence_order, allocated_hours, target_week_start, target_week_end, pedagogical_approach, assessment_plan
) VALUES
    (NULL, 'c0000002-0000-0000-0000-000000000001', '2026/2027', 'Kelas 5 SD', 'ODD', 1, 6, 1, 3, 'Inquiry Based Learning', 'Formatif Diagnostik & Rubrik Poster Organ'),
    (NULL, 'c0000002-0000-0000-0000-000000000002', '2026/2027', 'Kelas 5 SD', 'ODD', 2, 6, 4, 6, 'Problem-Based Learning', 'Formatif Presentasi Jaring Makanan & Tes Sumatif Lingkup Materi 1')
ON CONFLICT DO NOTHING;

-- 4.2 BAHASA INDONESIA FASE C
WITH inserted_cp_indo AS (
    INSERT INTO learning_outcomes (
        id, tenant_id, subject_code, subject_name, phase, target_grades, 
        element_name, element_code, description, is_ai_generated, is_verified, order_index
    ) VALUES (
        'c0000001-0000-0000-0000-000000000002',
        NULL,
        '300110000',
        'Bahasa Indonesia',
        'FASE_C',
        'Kelas 5-6 SD',
        'Membaca dan Memirsa',
        'ELEMEN_MEMBACA',
        'Peserta didik mampu membaca kata-kata dengan berbagai pola kombinasi huruf secara fasih dan indah serta memahami informasi dan kosakata baru yang memiliki makna denotatif, konotatif, dan kiasan untuk mengidentifikasi objek, fenomena, dan karakter tokoh dalam teks narasi fiksi dan teks informatif eksplanasi.',
        false,
        true,
        1
    ) ON CONFLICT (id) DO NOTHING
    RETURNING id
),
inserted_tp_indo_1 AS (
    INSERT INTO learning_objectives (
        id, tenant_id, learning_outcome_id, code, competency, bloom_level,
        content_scope, statement, pancasila_profiles, evidence_indicators, estimated_hours, order_index
    ) VALUES (
        'c0000002-0000-0000-0000-000000000003',
        NULL,
        'c0000001-0000-0000-0000-000000000002',
        'TP.BIND.5.1',
        'Menemukan & Menyimpulkan',
        'C3',
        'Ide Pokok & Informasi Teks Eksplanasi',
        'Peserta didik mampu menemukan ide pokok, kalimat pendukung, dan menyimpulkan isi teks eksplanasi ilmiah sederhana dengan kosakata baku.',
        ARRAY['Bernalar Kritis', 'Mandiri'],
        ARRAY['Menandai kalimat utama pada setiap paragraf', 'Menjelaskan hubungan sebab-akibat fenomena alam', 'Membuat ringkasan teks 100 kata dengan kalimat efektif'],
        6,
        1
    ) ON CONFLICT (id) DO NOTHING
    RETURNING id
)
INSERT INTO learning_objective_flows (
    tenant_id, learning_objective_id, academic_year, grade_level, semester,
    sequence_order, allocated_hours, target_week_start, target_week_end, pedagogical_approach, assessment_plan
) VALUES
    (NULL, 'c0000002-0000-0000-0000-000000000003', '2026/2027', 'Kelas 5 SD', 'ODD', 1, 6, 1, 3, 'Genre-Based Pedagogy', 'Formatif Ceklis Membaca & Lembar Kerja Analisis Paragraf')
ON CONFLICT DO NOTHING;
