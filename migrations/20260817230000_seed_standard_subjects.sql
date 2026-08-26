-- 20260817230000_seed_standard_subjects.sql
-- Migration to seed standard Dapodik subjects into the database

INSERT INTO subjects (id, tenant_id, code, name, is_active, created_at, updated_at)
SELECT gen_random_uuid(), t.id, v.code, v.name, true, NOW(), NOW()
FROM tenants t
CROSS JOIN (
    VALUES
    ('100011070', 'Pendidikan Agama Islam dan Budi Pekerti'),
    ('100012050', 'Pendidikan Agama Kristen dan Budi Pekerti'),
    ('100013010', 'Pendidikan Agama Katholik dan Budi Pekerti'),
    ('100014140', 'Pendidikan Agama Buddha dan Budi Pekerti'),
    ('100015010', 'Pendidikan Agama Hindu dan Budi Pekerti'),
    ('100016010', 'Pendidikan Agama Konghuchu dan Budi Pekerti'),
    ('109011000', 'Pendidikan Kepercayaan terhadap Tuhan YME'),
    ('200010300', 'Pendidikan Pancasila'),
    ('200040000', 'Pembelajaran Berbasis Projek'),
    ('300110000', 'Bahasa Indonesia'),
    ('401000000', 'Matematika (Umum)'),
    ('401900000', 'Ilmu Pengetahuan Alam dan Sosial (IPAS)'),
    ('500010000', 'Pendidikan Jasmani, Olahraga, dan Kesehatan'),
    ('700201010', 'PEMBELAJARAN MUATAN PEMBERDAYAAN'),
    ('700201020', 'PEMBELAJARAN MUATAN KETERAMPILAN ROBOTIKA'),
    ('700201030', 'PEMBELAJARAN MUATAN KETERAMPILAN PENGELOLAAN SAMPAH'),
    ('700201040', 'PEMBELAJARAN MUATAN KETERAMPILAN PERTANIAN TERPADU'),
    ('700201050', 'PEMBELAJARAN MUATAN KETERAMPILAN BARISTA'),
    ('700201060', 'PEMBELAJARAN MUATAN KETERAMPILAN PERIKANAN TANGKAP'),
    ('700201070', 'PEMBELAJARAN MUATAN KETERAMPILAN TATA BOGA'),
    ('700201080', 'PEMBELAJARAN MUATAN KETERAMPILAN TATA BUSANA'),
    ('700201090', 'PEMBELAJARAN MUATAN KETERAMPILAN KOMPUTER APLIKASI'),
    ('700201100', 'PEMBELAJARAN MUATAN KETERAMPILAN KREATOR KONTEN'),
    ('843020100', 'Seni Budaya'),
    ('999800413', 'Pemberdayaan'),
    ('999800414', 'Keterampilan'),
    ('300311900', 'Muatan Lokal Bahasa Daerah'),
    ('300312900', 'Muatan Lokal Potensi Daerah'),
    ('300210000', 'Bahasa Inggris')
) AS v(code, name)
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name, updated_at = NOW();
