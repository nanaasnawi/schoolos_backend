-- 20261012183000_create_kosp_ecosystem.sql
-- Kurikulum Operasional Satuan Pendidikan (KOSP) Ecosystem:
-- 1. kosp_documents: Metadata dokumen induk KOSP per tahun ajaran & status legalitas
-- 2. kosp_committees: Tim Pengembang Kurikulum (TPK) satuan pendidikan
-- 3. kosp_signatories: Pejabat penandatangan (Kepala Sekolah, Penilik PNF, Kabid PNF Disdik)

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. KOSP DOCUMENTS (DOKUMEN INDUK OPERASIONAL KURIKULUM SATUAN PENDIDIKAN)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS kosp_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    academic_year VARCHAR(50) NOT NULL, -- e.g. '2026/2027'
    title VARCHAR(255) NOT NULL,        -- 'Kurikulum Operasional Satuan Pendidikan (KOSP) PKBM As-Salafiyah'
    legal_basis_statement TEXT NOT NULL,
    establishment_place VARCHAR(100) NOT NULL DEFAULT 'Pabuaran',
    establishment_date DATE NOT NULL DEFAULT '2026-07-13',
    effective_weeks_odd INTEGER NOT NULL DEFAULT 18,
    effective_weeks_even INTEGER NOT NULL DEFAULT 17,
    status VARCHAR(50) NOT NULL DEFAULT 'RATIFIED', -- 'DRAFT', 'SUBMITTED', 'RATIFIED'
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    CONSTRAINT uq_kosp_tenant_year UNIQUE (tenant_id, academic_year)
);

CREATE INDEX IF NOT EXISTS idx_kosp_tenant_year ON kosp_documents(tenant_id, academic_year);

CREATE TRIGGER update_kosp_documents_updated_at
    BEFORE UPDATE ON kosp_documents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. KOSP COMMITTEES (TIM PENGEMBANG KURIKULUM / TPK SATUAN PENDIDIKAN)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS kosp_committees (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kosp_document_id UUID NOT NULL REFERENCES kosp_documents(id) ON DELETE CASCADE,
    teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL,
    name VARCHAR(150) NOT NULL,
    role_in_committee VARCHAR(100) NOT NULL, -- 'Ketua TPK', 'Tutor Paket C', 'Tutor Paket B', 'Tutor Paket A'
    order_index INTEGER NOT NULL DEFAULT 1,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_kosp_comm_doc ON kosp_committees(kosp_document_id);
CREATE INDEX IF NOT EXISTS idx_kosp_comm_teacher ON kosp_committees(teacher_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. KOSP SIGNATORIES (PEJABAT PENGESAH & LEGALITAS LEMBAR PENGESAHAN)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS kosp_signatories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kosp_document_id UUID NOT NULL REFERENCES kosp_documents(id) ON DELETE CASCADE,
    signatory_role VARCHAR(100) NOT NULL, -- 'KEPALA_SEKOLAH', 'PENILIK_KECAMATAN', 'KABID_PNF_DISDIK'
    position_title VARCHAR(150) NOT NULL, -- 'Kepala PKBM As-Salafiyah', 'Penilik PNF Kec. Pabuaran', 'Kepala Bidang PNF Dinas Pendidikan Kab. Cirebon'
    name VARCHAR(150) NOT NULL,
    nip VARCHAR(50),
    has_signature BOOLEAN NOT NULL DEFAULT true,
    has_official_seal BOOLEAN NOT NULL DEFAULT true,
    order_index INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_kosp_sign_doc ON kosp_signatories(kosp_document_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. SEED DATA RESMI KOSP PKBM AS-SALAFIYAH TAHUN AJARAN 2026/2027
-- ─────────────────────────────────────────────────────────────────────────────
DO $$
DECLARE
    v_tenant_id UUID;
    v_kosp_id UUID;
    v_t_rohmana UUID;
    v_t_kuswanto UUID;
    v_t_asyifa UUID;
    v_t_amin UUID;
BEGIN
    SELECT id INTO v_tenant_id FROM tenants WHERE name ILIKE '%As-Salafiyah%' LIMIT 1;
    IF v_tenant_id IS NULL THEN
        SELECT id INTO v_tenant_id FROM tenants LIMIT 1;
    END IF;

    IF v_tenant_id IS NOT NULL THEN
        -- Insert Master KOSP Document
        INSERT INTO kosp_documents (
            id, tenant_id, academic_year, title, legal_basis_statement, 
            establishment_place, establishment_date, effective_weeks_odd, effective_weeks_even, status
        ) VALUES (
            gen_random_uuid(),
            v_tenant_id,
            '2026/2027',
            'Kurikulum Operasional Satuan Pendidikan (KOSP) PKBM As-Salafiyah',
            'Kurikulum Operasional Satuan Pendidikan (KOSP) PKBM As-Salafiyah Tahun Pelajaran 2026/2027 telah diperiksa, disetujui, dan disahkan untuk digunakan sebagai pedoman penyelenggaraan pembelajaran.',
            'Pabuaran',
            '2026-07-13',
            18,
            17,
            'RATIFIED'
        )
        ON CONFLICT (tenant_id, academic_year) DO UPDATE
        SET title = EXCLUDED.title,
            legal_basis_statement = EXCLUDED.legal_basis_statement,
            establishment_place = EXCLUDED.establishment_place,
            establishment_date = EXCLUDED.establishment_date,
            status = 'RATIFIED',
            updated_at = NOW()
        RETURNING id INTO v_kosp_id;

        -- Resolve Teacher UUIDs
        SELECT id INTO v_t_rohmana FROM teachers WHERE tenant_id = v_tenant_id AND full_name ILIKE '%ROHMANA%' LIMIT 1;
        SELECT id INTO v_t_kuswanto FROM teachers WHERE tenant_id = v_tenant_id AND full_name ILIKE '%KUSWANTO%' LIMIT 1;
        SELECT id INTO v_t_asyifa FROM teachers WHERE tenant_id = v_tenant_id AND full_name ILIKE '%ASY%SYIFA%' LIMIT 1;
        SELECT id INTO v_t_amin FROM teachers WHERE tenant_id = v_tenant_id AND full_name ILIKE '%AMIN LISANA%' LIMIT 1;

        -- Clean up existing committees & signatories for idempotent seed
        DELETE FROM kosp_committees WHERE kosp_document_id = v_kosp_id;
        DELETE FROM kosp_signatories WHERE kosp_document_id = v_kosp_id;

        -- Insert TPK
        INSERT INTO kosp_committees (kosp_document_id, teacher_id, name, role_in_committee, order_index)
        VALUES
            (v_kosp_id, v_t_rohmana, 'ROHMANA, S.P', 'Ketua TPK', 1),
            (v_kosp_id, v_t_kuswanto, 'KUSWANTO ADI S. S.Pd', 'Tutor Paket C', 2),
            (v_kosp_id, v_t_asyifa, 'ASYIFA RAHMA IHSANI, S.Pd', 'Tutor Paket B', 3),
            (v_kosp_id, v_t_amin, 'AMIN LISANA, S.Pd', 'Tutor Paket A', 4);

        -- Insert Signatories
        INSERT INTO kosp_signatories (kosp_document_id, signatory_role, position_title, name, nip, has_signature, has_official_seal, order_index)
        VALUES
            (v_kosp_id, 'KEPALA_SEKOLAH', 'Kepala PKBM As-Salafiyah', 'SITI MUNIROH, S.Pd', NULL, true, true, 1),
            (v_kosp_id, 'PENILIK_KECAMATAN', 'Penilik PNF Kec. Pabuaran', 'ARIEKA SUTRISNO, S.Pd', '19700123 200112 1 002', true, false, 2),
            (v_kosp_id, 'KABID_PNF_DISDIK', 'A.n. Kepala Dinas Pendidikan Kab. Cirebon - Kepala Bidang PNF', 'HERDILAH, S.AP, M.Pd', '19720428 199403 2 002', true, true, 3);
    END IF;
END $$;
