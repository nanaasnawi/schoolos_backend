-- 20261018040000_add_dapodik_pip_and_photos.sql
-- 1. Add foto_profile to students, teachers, staff for direct database photo persistence
ALTER TABLE students ADD COLUMN IF NOT EXISTS foto_profile TEXT;
ALTER TABLE teachers ADD COLUMN IF NOT EXISTS foto_profile TEXT;
ALTER TABLE staff ADD COLUMN IF NOT EXISTS foto_profile TEXT;

-- 2. Add complete Dapodik address, parental, and PIP (Program Indonesia Pintar) columns to students table
ALTER TABLE students 
    ADD COLUMN IF NOT EXISTS rt VARCHAR(10),
    ADD COLUMN IF NOT EXISTS rw VARCHAR(10),
    ADD COLUMN IF NOT EXISTS dusun VARCHAR(150),
    ADD COLUMN IF NOT EXISTS kelurahan VARCHAR(150),
    ADD COLUMN IF NOT EXISTS kecamatan VARCHAR(150),
    ADD COLUMN IF NOT EXISTS kode_pos VARCHAR(10),
    ADD COLUMN IF NOT EXISTS pekerjaan_ayah VARCHAR(150),
    ADD COLUMN IF NOT EXISTS penghasilan_ayah VARCHAR(150),
    ADD COLUMN IF NOT EXISTS pekerjaan_ibu VARCHAR(150),
    ADD COLUMN IF NOT EXISTS penghasilan_ibu VARCHAR(150),
    ADD COLUMN IF NOT EXISTS nama_wali VARCHAR(255),
    ADD COLUMN IF NOT EXISTS pekerjaan_wali VARCHAR(150),
    ADD COLUMN IF NOT EXISTS penghasilan_wali VARCHAR(150),
    ADD COLUMN IF NOT EXISTS penerima_kps BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS no_kps VARCHAR(100),
    ADD COLUMN IF NOT EXISTS penerima_kip BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS nomor_kip VARCHAR(100),
    ADD COLUMN IF NOT EXISTS nama_di_kip VARCHAR(255),
    ADD COLUMN IF NOT EXISTS nomor_kks VARCHAR(100),
    ADD COLUMN IF NOT EXISTS layak_pip BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS alasan_layak_pip TEXT,
    ADD COLUMN IF NOT EXISTS bank VARCHAR(100),
    ADD COLUMN IF NOT EXISTS nomor_rekening_bank VARCHAR(100),
    ADD COLUMN IF NOT EXISTS rekening_atas_nama VARCHAR(255);

-- 3. High-performance partial indices for PIP dashboard & monitoring
CREATE INDEX IF NOT EXISTS idx_students_tenant_penerima_kip ON students(tenant_id, penerima_kip) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_students_tenant_layak_pip ON students(tenant_id, layak_pip) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_students_tenant_bank ON students(tenant_id, bank) WHERE deleted_at IS NULL;
