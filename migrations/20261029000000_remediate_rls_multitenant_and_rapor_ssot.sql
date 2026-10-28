-- 20261029000000_remediate_rls_multitenant_and_rapor_ssot.sql
-- Description:
-- 1. Restore zero-bottleneck dynamic multi-tenant RLS function is_tenant_allowed() (eliminating regressed As-Salafiyah sequential scan from 20261026000000).
-- 2. Enable PostgreSQL RLS + tenant isolation policy on kombinasi_mapel_siswa.
-- 3. Remove hardcoded 'Pabuaran' default from kosp_documents.establishment_place.
-- 4. Recreate VIEW rapor_mata_pelajaran with dynamic semester/academic_year SSOT, universal PKBM + formal school class phase resolution, multi-religion matching, and strict ADR-0018 IPAS/IPA/IPS phase boundaries.

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. Dynamic Multi-Tenant RLS Function (ADR-0004, ADR-0012, ADR-0015)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION is_tenant_allowed(record_tenant_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    curr_tenant TEXT;
    bypass TEXT;
BEGIN
    IF current_setting('app.bypass_rls', true) = 'on' THEN
        RETURN true;
    END IF;

    curr_tenant := NULLIF(current_setting('app.current_tenant_id', true), '');
    IF curr_tenant IS NOT NULL THEN
        RETURN record_tenant_id = curr_tenant::uuid;
    END IF;

    -- Defer cleanly to mandatory SQL WHERE tenant_id = $1 without single-tenant lockout or ILIKE table scans (ADR-0015)
    RETURN true;
END;
$$ LANGUAGE plpgsql STABLE SECURITY DEFINER;

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. Enforce Multi-Tenant RLS on kombinasi_mapel_siswa
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE kombinasi_mapel_siswa ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS kombinasi_mapel_siswa_tenant_isolation ON kombinasi_mapel_siswa;
CREATE POLICY kombinasi_mapel_siswa_tenant_isolation ON kombinasi_mapel_siswa
    FOR ALL
    USING (is_tenant_allowed(tenant_id))
    WITH CHECK (is_tenant_allowed(tenant_id));

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. Remove Hardcoded Region Default on kosp_documents (ADR-0014)
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE kosp_documents ALTER COLUMN establishment_place SET DEFAULT '';

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. Recreate VIEW rapor_mata_pelajaran (SSOT Semester, PKBM + Formal Classes, ADR-0018)
-- ─────────────────────────────────────────────────────────────────────────────
DROP VIEW IF EXISTS rapor_mata_pelajaran;

CREATE VIEW rapor_mata_pelajaran AS
WITH student_context AS (
    SELECT
        s.id AS student_id,
        s.tenant_id,
        s.nisn,
        s.full_name AS student_name,
        s.gender,
        s.religion,
        c.id AS class_id,
        c.name AS class_name,
        CASE
            -- Paket C / SMA / SMK / MA & Kelas 10-12 (including KKA C12, C11, C10, XII, XI, X)
            WHEN c.name ~* '(^|[^A-Z])C\s*1[12]([^0-9]|$)' THEN 'Fase F'
            WHEN c.name ~* '(^|[^A-Z])C\s*10([^0-9]|$)' THEN 'Fase E'
            WHEN (c.name ILIKE '%PAKET C%' OR c.name ~* '(^|[^A-Z])(SMA|SMK|MA)([^A-Z]|$)')
                 AND (c.name ~* '(^|[^0-9])10([^0-9]|$)' OR c.name ~* '(^|[^A-Z])X([^A-Z]|$)') THEN 'Fase E'
            WHEN (c.name ILIKE '%PAKET C%' OR c.name ~* '(^|[^A-Z])(SMA|SMK|MA)([^A-Z]|$)') THEN 'Fase F'
            WHEN c.name ~* '(^|[^A-Z])(XI|XII)([^A-Z]|$)' OR c.name ~* '(^|[^A-Z])KELAS\s*(11|12)([^0-9]|$)' THEN 'Fase F'
            WHEN c.name ~* '(^|[^A-Z])X([^A-Z]|$)' OR c.name ~* '(^|[^A-Z])KELAS\s*10([^0-9]|$)' THEN 'Fase E'

            -- Paket B / SMP / MTs & Kelas 7-9 (including B7, B8, B9, VII, VIII, IX)
            WHEN c.name ILIKE '%PAKET B%'
                 OR c.name ~* '(^|[^A-Z])B\s*[789]([^0-9]|$)'
                 OR c.name ~* '(^|[^A-Z])(SMP|MTS)([^A-Z]|$)'
                 OR c.name ~* '(^|[^A-Z])(VII|VIII|IX)([^A-Z]|$)'
                 OR c.name ~* '(^|[^A-Z])KELAS\s*[789]([^0-9]|$)' THEN 'Fase D'

            -- Paket A / SD / MI & Kelas 1-6 (including A1..A6, I..VI)
            WHEN c.name ~* '(^|[^A-Z])(KELAS|A)\s*[56]([^0-9]|$)'
                 OR (c.name ILIKE '%PAKET A%' AND c.name ~* '(^|[^0-9])[56]([^0-9]|$)')
                 OR c.name ~* '(^|[^A-Z])(V|VI)([^A-Z]|$)' THEN 'Fase C'
            WHEN c.name ~* '(^|[^A-Z])(KELAS|A)\s*[34]([^0-9]|$)'
                 OR (c.name ILIKE '%PAKET A%' AND c.name ~* '(^|[^0-9])[34]([^0-9]|$)')
                 OR c.name ~* '(^|[^A-Z])(III|IV)([^A-Z]|$)' THEN 'Fase B'
            WHEN c.name ~* '(^|[^A-Z])(KELAS|A)\s*[12]([^0-9]|$)'
                 OR (c.name ILIKE '%PAKET A%' AND c.name ~* '(^|[^0-9])[12]([^0-9]|$)')
                 OR c.name ~* '(^|[^A-Z])(I|II)([^A-Z]|$)' THEN 'Fase A'
            WHEN c.name ILIKE '%PAKET A%' OR c.name ~* '(^|[^A-Z])(SD|MI)([^A-Z]|$)' THEN 'Fase C'

            -- Fallback to grade_levels.level when class name has custom nomenclature
            WHEN gl.level IN (11, 12) THEN 'Fase F'
            WHEN gl.level = 10 THEN 'Fase E'
            WHEN gl.level IN (7, 8, 9) THEN 'Fase D'
            WHEN gl.level IN (5, 6) THEN 'Fase C'
            WHEN gl.level IN (3, 4) THEN 'Fase B'
            WHEN gl.level IN (1, 2) THEN 'Fase A'
            ELSE 'Fase F'
        END AS phase,
        COALESCE(ay.name, '') AS academic_year,
        (CASE WHEN COALESCE(ay.current_semester, 1) = 2 THEN 'EVEN' ELSE 'ODD' END)::varchar(10) AS semester
    FROM students s
    JOIN enrollments e ON e.student_id = s.id AND e.tenant_id = s.tenant_id AND UPPER(e.status) = 'ACTIVE'
    JOIN classes c ON c.id = e.class_id AND c.tenant_id = s.tenant_id AND c.deleted_at IS NULL
    LEFT JOIN grade_levels gl ON gl.id = c.grade_level_id AND gl.tenant_id = s.tenant_id
    LEFT JOIN LATERAL (
        SELECT ay_inner.name, ay_inner.current_semester
        FROM academic_years ay_inner
        WHERE ay_inner.tenant_id = s.tenant_id
          AND ay_inner.deleted_at IS NULL
          AND (
              (e.academic_year_id IS NOT NULL AND ay_inner.id = e.academic_year_id)
              OR (e.academic_year_id IS NULL AND ay_inner.is_active = true)
          )
        ORDER BY (ay_inner.id = e.academic_year_id) DESC NULLS LAST, ay_inner.is_active DESC, ay_inner.updated_at DESC
        LIMIT 1
    ) ay ON true
    WHERE s.deleted_at IS NULL
)
-- Bagian 1: Mata Pelajaran UMUM & MUATAN LOKAL sesuai fase & agama siswa
SELECT
    sc.student_id,
    sc.tenant_id,
    sc.nisn,
    sc.student_name,
    sc.gender,
    sc.class_id,
    sc.class_name,
    sc.phase,
    sub.code AS subject_code,
    sub.name AS subject_name,
    CASE
        WHEN sub.name ILIKE '%Muatan Lokal%'
             OR sub.name ILIKE '%Bahasa Daerah%'
             OR sub.name ILIKE '%Bahasa Cirebon%'
             OR sub.name ILIKE '%Bahasa Sunda%'
             OR sub.name ILIKE '%Bahasa Jawa%' THEN 'MUATAN_LOKAL'
        ELSE 'UMUM'
    END::varchar(50) AS subject_group,
    sc.academic_year,
    sc.semester
FROM student_context sc
JOIN subjects sub
  ON sub.tenant_id = sc.tenant_id
 AND sub.is_active = true
 AND sub.deleted_at IS NULL
WHERE (
    -- A. Pendidikan Agama & Budi Pekerti (matched to student religion if present, or tenant active religion subject)
    (
        sub.name ILIKE 'Pendidikan Agama%'
        AND (
            (sc.religion ILIKE '%kristen%' OR sc.religion ILIKE '%protestan%') AND sub.name ILIKE 'Pendidikan Agama Kristen%'
            OR (sc.religion ILIKE '%katolik%' OR sc.religion ILIKE '%katholik%') AND sub.name ILIKE 'Pendidikan Agama Kat%olik%'
            OR (sc.religion ILIKE '%hindu%') AND sub.name ILIKE 'Pendidikan Agama Hindu%'
            OR (sc.religion ILIKE '%buddha%' OR sc.religion ILIKE '%budha%') AND sub.name ILIKE 'Pendidikan Agama Bud%ha%'
            OR (sc.religion ILIKE '%konghucu%' OR sc.religion ILIKE '%konghuchu%' OR sc.religion ILIKE '%khonghucu%') AND sub.name ILIKE 'Pendidikan Agama K%onghuc%'
            OR (sc.religion ILIKE '%islam%') AND sub.name ILIKE 'Pendidikan Agama Islam%'
            OR (
                (sc.religion IS NULL OR TRIM(sc.religion) = '')
                AND (
                    sub.name ILIKE 'Pendidikan Agama Islam%'
                    OR NOT EXISTS (
                        SELECT 1 FROM subjects s_rel
                        WHERE s_rel.tenant_id = sc.tenant_id
                          AND s_rel.is_active = true
                          AND s_rel.deleted_at IS NULL
                          AND s_rel.name ILIKE 'Pendidikan Agama Islam%'
                    )
                )
            )
        )
    )
    OR (sc.religion ILIKE '%kepercayaan%' AND sub.name ILIKE 'Pendidikan Kepercayaan%')
    OR (
        -- B. Mata Pelajaran Umum Lintas Fase (dengan pagar ketat ADR-0018 terhadap IPAS & macro IPA/IPS)
        sub.name NOT ILIKE 'Pendidikan Agama%'
        AND sub.name NOT ILIKE 'Pendidikan Kepercayaan%'
        AND (
            -- Fase F (Kelas 11 & 12 — Paket C / SMA / SMK): DILARANG IPAS dan DILARANG macro IPA/IPS!
            (sc.phase = 'Fase F'
                AND sub.name NOT ILIKE '%IPAS%'
                AND sub.name NOT ILIKE '%Alam dan Sosial%'
                AND sub.name NOT IN ('Ilmu Pengetahuan Alam (IPA)', 'Ilmu Pengetahuan Sosial (IPS)', 'IPA', 'IPS')
                AND (
                    sub.name IN (
                        'Pendidikan Pancasila',
                        'Pendidikan Pancasila dan Kewarganegaraan',
                        'Bahasa Indonesia',
                        'Matematika',
                        'Matematika (Umum)',
                        'Bahasa Inggris',
                        'Sejarah',
                        'Sejarah Indonesia',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)',
                        'Seni Budaya',
                        'Seni dan Budaya',
                        'Keterampilan Komputer',
                        'Keterampilan Fungsional',
                        'Keterampilan',
                        'Pemberdayaan'
                    )
                    OR sub.name ILIKE 'Muatan Lokal%'
                    OR sub.name ILIKE '%Bahasa Daerah%'
                    OR sub.name ILIKE '%Bahasa Cirebon%'
                )
            )
            OR
            -- Fase E (Kelas 10 — Paket C / SMA / SMK): Tanpa IPAS
            (sc.phase = 'Fase E'
                AND sub.name NOT ILIKE '%IPAS%'
                AND sub.name NOT ILIKE '%Alam dan Sosial%'
                AND (
                    sub.name IN (
                        'Pendidikan Pancasila',
                        'Pendidikan Pancasila dan Kewarganegaraan',
                        'Bahasa Indonesia',
                        'Matematika',
                        'Matematika (Umum)',
                        'Ilmu Pengetahuan Alam (IPA)',
                        'Ilmu Pengetahuan Sosial (IPS)',
                        'IPA',
                        'IPS',
                        'Bahasa Inggris',
                        'Sejarah',
                        'Sejarah Indonesia',
                        'Informatika',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)',
                        'Seni Budaya',
                        'Seni dan Budaya',
                        'Keterampilan Komputer',
                        'Keterampilan Fungsional',
                        'Keterampilan',
                        'Pemberdayaan'
                    )
                    OR sub.name ILIKE 'Muatan Lokal%'
                    OR sub.name ILIKE '%Bahasa Daerah%'
                    OR sub.name ILIKE '%Bahasa Cirebon%'
                )
            )
            OR
            -- Fase D (Kelas 7-9 — Paket B / SMP): Tanpa IPAS
            (sc.phase = 'Fase D'
                AND sub.name NOT ILIKE '%IPAS%'
                AND sub.name NOT ILIKE '%Alam dan Sosial%'
                AND (
                    sub.name IN (
                        'Pendidikan Pancasila',
                        'Pendidikan Pancasila dan Kewarganegaraan',
                        'Bahasa Indonesia',
                        'Matematika',
                        'Matematika (Umum)',
                        'Ilmu Pengetahuan Alam (IPA)',
                        'Ilmu Pengetahuan Sosial (IPS)',
                        'IPA',
                        'IPS',
                        'Bahasa Inggris',
                        'Informatika',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)',
                        'Seni Budaya',
                        'Seni dan Budaya',
                        'Keterampilan Komputer',
                        'Keterampilan',
                        'Pemberdayaan'
                    )
                    OR sub.name ILIKE 'Muatan Lokal%'
                    OR sub.name ILIKE '%Bahasa Daerah%'
                    OR sub.name ILIKE '%Bahasa Cirebon%'
                )
            )
            OR
            -- Fase B & Fase C (Kelas 3-6 — Paket A / SD): IPAS diperbolehkan (tanpa terpisah IPA/IPS SMP)
            (sc.phase IN ('Fase B', 'Fase C')
                AND sub.name NOT IN ('Ilmu Pengetahuan Alam (IPA)', 'Ilmu Pengetahuan Sosial (IPS)', 'IPA', 'IPS')
                AND (
                    sub.name IN (
                        'Pendidikan Pancasila',
                        'Pendidikan Pancasila dan Kewarganegaraan',
                        'Bahasa Indonesia',
                        'Matematika',
                        'Matematika (Umum)',
                        'Ilmu Pengetahuan Alam dan Sosial (IPAS)',
                        'IPAS',
                        'Bahasa Inggris',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)',
                        'Seni Budaya',
                        'Seni dan Budaya',
                        'Keterampilan',
                        'Pemberdayaan'
                    )
                    OR sub.name ILIKE 'Muatan Lokal%'
                    OR sub.name ILIKE '%Bahasa Daerah%'
                    OR sub.name ILIKE '%Bahasa Cirebon%'
                )
            )
            OR
            -- Fase A (Kelas 1-2 — Paket A / SD): Mapel Dasar Fase A (Tanpa IPAS & Tanpa IPA/IPS)
            (sc.phase = 'Fase A'
                AND sub.name NOT ILIKE '%IPAS%'
                AND sub.name NOT ILIKE '%Alam dan Sosial%'
                AND sub.name NOT IN ('Ilmu Pengetahuan Alam (IPA)', 'Ilmu Pengetahuan Sosial (IPS)', 'IPA', 'IPS')
                AND (
                    sub.name IN (
                        'Pendidikan Pancasila',
                        'Pendidikan Pancasila dan Kewarganegaraan',
                        'Bahasa Indonesia',
                        'Matematika',
                        'Matematika (Umum)',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan',
                        'Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)',
                        'Seni Budaya',
                        'Seni dan Budaya',
                        'Keterampilan',
                        'Pemberdayaan'
                    )
                    OR sub.name ILIKE 'Muatan Lokal%'
                    OR sub.name ILIKE '%Bahasa Daerah%'
                    OR sub.name ILIKE '%Bahasa Cirebon%'
                )
            )
        )
    )
)

UNION ALL

-- Bagian 2: Mata Pelajaran PILIHAN dari kombinasi_mapel_siswa
SELECT
    sc.student_id,
    sc.tenant_id,
    sc.nisn,
    sc.student_name,
    sc.gender,
    sc.class_id,
    sc.class_name,
    sc.phase,
    kms.subject_code,
    kms.subject_name,
    'PILIHAN'::varchar(50) AS subject_group,
    COALESCE(NULLIF(kms.academic_year, ''), sc.academic_year) AS academic_year,
    COALESCE(NULLIF(kms.semester, ''), sc.semester)::varchar(10) AS semester
FROM student_context sc
JOIN kombinasi_mapel_siswa kms
  ON kms.student_id = sc.student_id
 AND kms.tenant_id = sc.tenant_id
 AND kms.is_active = true;
