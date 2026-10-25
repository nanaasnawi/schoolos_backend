-- 20261025020000_add_headmaster_and_regional_metadata_to_schools.sql
-- ADR-0019: Dynamic School SSOT, Zero-Hardcode Architecture, & Backend-Centric Report Card Engine

-- 1. Tambah metadata pimpinan satuan pendidikan dan wilayah administratif terstruktur
ALTER TABLE schools ADD COLUMN IF NOT EXISTS headmaster_name VARCHAR(255);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS headmaster_nip VARCHAR(50);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS village VARCHAR(100);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS district VARCHAR(100);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS regency VARCHAR(100);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS province VARCHAR(100);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS postal_code VARCHAR(20);
ALTER TABLE schools ADD COLUMN IF NOT EXISTS website VARCHAR(255);

-- 2. Sinkronisasi data resmi Kepala Sekolah & Wilayah ke profil sekolah PKBM As-Salafiyah
UPDATE schools SET 
    headmaster_name = 'SITI MUNIROH',
    headmaster_nip = '-',
    village = 'Pabuaran Wetan',
    district = 'Pabuaran',
    regency = 'Cirebon',
    province = 'Jawa Barat',
    postal_code = '45196',
    website = 'https://pkbm-assalafiyah.sch.id'
WHERE tenant_id = 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0'
  AND (headmaster_name IS NULL OR headmaster_name = '');

-- 3. Sinkronisasi SSOT Tahun Ajaran 2026/2027 Aktif di database
INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'e3c4d8da-ff44-4d87-bb13-759a54f49bd0',
    '2026/2027',
    '2026-07-01',
    '2027-06-30',
    true,
    NOW(),
    NOW()
)
ON CONFLICT DO NOTHING;

-- Pastikan tahun ajaran 2026/2027 berstatus aktif utama untuk tenant PKBM As-Salafiyah
UPDATE academic_years 
SET is_active = (name = '2026/2027')
WHERE tenant_id = 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0';
