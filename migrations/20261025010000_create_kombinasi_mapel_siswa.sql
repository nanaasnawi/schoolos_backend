-- 20261025010000_create_kombinasi_mapel_siswa.sql
-- Kurikulum Merdeka Kemendikdasmen RI: Struktur Mata Pelajaran & Peminatan Fase F (Paket C)
-- 1. kombinasi_mapel_siswa: Kombinasi mata pelajaran pilihan siswa Fase F
-- 2. rapor_mata_pelajaran: View relasional filter mapel spesifik per rombel & fase

CREATE TABLE IF NOT EXISTS kombinasi_mapel_siswa (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID REFERENCES classes(id) ON DELETE CASCADE,
    academic_year VARCHAR(50) NOT NULL DEFAULT '2026/2027',
    semester VARCHAR(10) NOT NULL DEFAULT 'ODD', -- 'ODD', 'EVEN'
    subject_code VARCHAR(50) NOT NULL,
    subject_name VARCHAR(150) NOT NULL,
    subject_group VARCHAR(50) NOT NULL DEFAULT 'PILIHAN', -- 'UMUM', 'PILIHAN', 'MUATAN_LOKAL'
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    CONSTRAINT uq_kombinasi_student_mapel UNIQUE(tenant_id, student_id, academic_year, semester, subject_name)
);

CREATE INDEX IF NOT EXISTS idx_kms_student ON kombinasi_mapel_siswa(tenant_id, student_id, academic_year);
CREATE INDEX IF NOT EXISTS idx_kms_class ON kombinasi_mapel_siswa(tenant_id, class_id);

-- Seed kombinasi mapel pilihan resmi Kurikulum Merdeka untuk siswa Paket C Fase F
DO $$
DECLARE
    v_tenant_id UUID;
    v_student RECORD;
    v_class_c12 UUID;
BEGIN
    SELECT id INTO v_tenant_id FROM tenants WHERE name ILIKE '%As-Salafiyah%' LIMIT 1;
    IF v_tenant_id IS NULL THEN
        SELECT id INTO v_tenant_id FROM tenants LIMIT 1;
    END IF;

    IF v_tenant_id IS NOT NULL THEN
        SELECT id INTO v_class_c12 FROM classes 
        WHERE tenant_id = v_tenant_id AND name ILIKE '%PAKET C%12%' LIMIT 1;

        -- Ambil seluruh siswa Paket C (misal Abdul Aris, Abdul Muis, dll)
        FOR v_student IN 
            SELECT s.id, s.full_name, c.id AS cid
            FROM students s
            JOIN enrollments e ON e.student_id = s.id AND e.status = 'Active'
            JOIN classes c ON c.id = e.class_id
            WHERE s.tenant_id = v_tenant_id AND (c.name ILIKE '%PAKET C%' OR s.rombel ILIKE '%PAKET C%')
        LOOP
            -- Mapel Pilihan 1: Sosiologi
            INSERT INTO kombinasi_mapel_siswa (
                tenant_id, student_id, class_id, academic_year, semester,
                subject_code, subject_name, subject_group, is_active
            ) VALUES (
                v_tenant_id, v_student.id, v_student.cid, '2026/2027', 'ODD',
                'P-SOS-01', 'Sosiologi', 'PILIHAN', true
            ) ON CONFLICT (tenant_id, student_id, academic_year, semester, subject_name) DO NOTHING;

            -- Mapel Pilihan 2: Ekonomi
            INSERT INTO kombinasi_mapel_siswa (
                tenant_id, student_id, class_id, academic_year, semester,
                subject_code, subject_name, subject_group, is_active
            ) VALUES (
                v_tenant_id, v_student.id, v_student.cid, '2026/2027', 'ODD',
                'P-EKO-01', 'Ekonomi', 'PILIHAN', true
            ) ON CONFLICT (tenant_id, student_id, academic_year, semester, subject_name) DO NOTHING;
        END LOOP;
    END IF;
END $$;

-- 2. View rapor_mata_pelajaran: Menggabungkan Mapel Wajib (UMUM & MULOK) dan Mapel Pilihan (PILIHAN)
--    Menjamin tidak terjadi kekosongan mapel umum dan tidak ada tumpang tindih mapel non-pilihan
DROP VIEW IF EXISTS rapor_mata_pelajaran;

CREATE VIEW rapor_mata_pelajaran AS
-- Bagian 1: Mata Pelajaran UMUM & MUATAN LOKAL (Wajib sesuai tingkatan fase & rombel)
SELECT 
    s.id AS student_id,
    s.tenant_id,
    s.nisn,
    s.full_name AS student_name,
    s.gender,
    c.id AS class_id,
    c.name AS class_name,
    CASE 
        WHEN c.name ILIKE '%PAKET C%12%' OR c.name ILIKE '%PAKET C%11%' THEN 'Fase F'
        WHEN c.name ILIKE '%PAKET C%10%' THEN 'Fase E'
        WHEN c.name ILIKE '%PAKET B%' THEN 'Fase D'
        WHEN c.name ILIKE '%PAKET A%5%' OR c.name ILIKE '%PAKET A%6%' THEN 'Fase C'
        WHEN c.name ILIKE '%PAKET A%3%' OR c.name ILIKE '%PAKET A%4%' THEN 'Fase B'
        WHEN c.name ILIKE '%PAKET A%1%' OR c.name ILIKE '%PAKET A%2%' THEN 'Fase A'
        ELSE 'Fase F'
    END AS phase,
    sub.code AS subject_code,
    sub.name AS subject_name,
    CASE 
        WHEN sub.name ILIKE '%Muatan Lokal%' OR sub.name ILIKE '%Bahasa Cirebon%' THEN 'MUATAN_LOKAL'
        ELSE 'UMUM'
    END AS subject_group,
    COALESCE(ay.name, '2026/2027') AS academic_year,
    'ODD'::varchar(10) AS semester
FROM students s
JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'ACTIVE')
JOIN classes c ON c.id = e.class_id
LEFT JOIN academic_years ay ON ay.id = e.academic_year_id
JOIN subjects sub ON sub.tenant_id = s.tenant_id AND sub.is_active = true AND sub.deleted_at IS NULL
WHERE (
    -- Paket C Fase F (Kelas 11 & 12): Mapel Umum Wajib, DILARANG IPAS & macro IPA/IPS
    (c.name ILIKE '%PAKET C%12%' OR c.name ILIKE '%PAKET C%11%') AND sub.name IN (
        'Pendidikan Agama Islam dan Budi Pekerti',
        'Pendidikan Pancasila',
        'Bahasa Indonesia',
        'Matematika (Umum)',
        'Bahasa Inggris',
        'Sejarah',
        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
        'Seni Budaya',
        'Keterampilan Komputer',
        'Muatan Lokal Bahasa Daerah'
    )
    OR
    -- Paket C Fase E (Kelas 10): Mapel Dasar Tanpa IPAS
    (c.name ILIKE '%PAKET C%10%') AND sub.name IN (
        'Pendidikan Agama Islam dan Budi Pekerti',
        'Pendidikan Pancasila',
        'Bahasa Indonesia',
        'Matematika (Umum)',
        'Ilmu Pengetahuan Alam (IPA)',
        'Ilmu Pengetahuan Sosial (IPS)',
        'Bahasa Inggris',
        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
        'Seni Budaya',
        'Keterampilan Komputer',
        'Muatan Lokal Bahasa Daerah'
    )
    OR
    -- Paket B (Fase D / SMP): Mapel SMP Tanpa IPAS
    (c.name ILIKE '%PAKET B%') AND sub.name IN (
        'Pendidikan Agama Islam dan Budi Pekerti',
        'Pendidikan Pancasila',
        'Bahasa Indonesia',
        'Matematika (Umum)',
        'Ilmu Pengetahuan Alam (IPA)',
        'Ilmu Pengetahuan Sosial (IPS)',
        'Bahasa Inggris',
        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
        'Seni Budaya',
        'Keterampilan Komputer',
        'Muatan Lokal Bahasa Daerah'
    )
    OR
    -- Paket A (Fase A/B/C / SD): IPAS HANYA di Kelas 3-6
    (c.name ILIKE '%PAKET A%') AND (
        ((c.name ILIKE '%PAKET A%1%' OR c.name ILIKE '%PAKET A%2%') AND sub.name IN (
            'Pendidikan Agama Islam dan Budi Pekerti',
            'Pendidikan Pancasila',
            'Bahasa Indonesia',
            'Matematika (Umum)',
            'Pendidikan Jasmani, Olahraga, dan Kesehatan',
            'Seni Budaya',
            'Muatan Lokal Bahasa Daerah'
        ))
        OR
        ((c.name NOT ILIKE '%PAKET A%1%' AND c.name NOT ILIKE '%PAKET A%2%') AND sub.name IN (
            'Pendidikan Agama Islam dan Budi Pekerti',
            'Pendidikan Pancasila',
            'Bahasa Indonesia',
            'Matematika (Umum)',
            'Ilmu Pengetahuan Alam dan Sosial (IPAS)',
            'Pendidikan Jasmani, Olahraga, dan Kesehatan',
            'Seni Budaya',
            'Bahasa Inggris',
            'Muatan Lokal Bahasa Daerah'
        ))
    )
)

UNION ALL

-- Bagian 2: Mata Pelajaran PILIHAN (Hanya untuk siswa yang mengambil mapel pilihan dari kombinasi_mapel_siswa)
SELECT 
    s.id AS student_id,
    s.tenant_id,
    s.nisn,
    s.full_name AS student_name,
    s.gender,
    c.id AS class_id,
    c.name AS class_name,
    CASE 
        WHEN c.name ILIKE '%PAKET C%12%' OR c.name ILIKE '%PAKET C%11%' THEN 'Fase F'
        WHEN c.name ILIKE '%PAKET C%10%' THEN 'Fase E'
        WHEN c.name ILIKE '%PAKET B%' THEN 'Fase D'
        WHEN c.name ILIKE '%PAKET A%5%' OR c.name ILIKE '%PAKET A%6%' THEN 'Fase C'
        WHEN c.name ILIKE '%PAKET A%3%' OR c.name ILIKE '%PAKET A%4%' THEN 'Fase B'
        WHEN c.name ILIKE '%PAKET A%1%' OR c.name ILIKE '%PAKET A%2%' THEN 'Fase A'
        ELSE 'Fase F'
    END AS phase,
    kms.subject_code,
    kms.subject_name,
    'PILIHAN'::varchar(50) AS subject_group,
    kms.academic_year,
    kms.semester
FROM students s
JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'ACTIVE')
JOIN classes c ON c.id = e.class_id
JOIN kombinasi_mapel_siswa kms ON kms.student_id = s.id AND kms.is_active = true;

