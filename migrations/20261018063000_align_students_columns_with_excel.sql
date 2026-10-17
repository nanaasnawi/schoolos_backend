-- 20261018063000_align_students_columns_with_excel.sql
-- Align PostgreSQL students table columns with official Dapodik Excel format ("data peserta didik pkbm as-salafiyah fix.xlsx")
-- Guarantees zero "column does not exist" errors whether queried using Excel headers or canonical names.

ALTER TABLE students
    ADD COLUMN IF NOT EXISTS nama VARCHAR(255),
    ADD COLUMN IF NOT EXISTS jk VARCHAR(20),
    ADD COLUMN IF NOT EXISTS tempat_lahir VARCHAR(150),
    ADD COLUMN IF NOT EXISTS tanggal_lahir DATE,
    ADD COLUMN IF NOT EXISTS agama VARCHAR(50),
    ADD COLUMN IF NOT EXISTS alamat TEXT,
    ADD COLUMN IF NOT EXISTS hp VARCHAR(50),
    ADD COLUMN IF NOT EXISTS nomor_kps VARCHAR(100),
    ADD COLUMN IF NOT EXISTS data_ayah VARCHAR(255),
    ADD COLUMN IF NOT EXISTS data_ibu VARCHAR(255),
    ADD COLUMN IF NOT EXISTS rombel VARCHAR(150),
    ADD COLUMN IF NOT EXISTS rombel_saat_ini VARCHAR(150);

-- Sync existing data
UPDATE students SET
    nama = COALESCE(nama, full_name),
    jk = COALESCE(jk, gender),
    tempat_lahir = COALESCE(tempat_lahir, place_of_birth),
    tanggal_lahir = COALESCE(tanggal_lahir, date_of_birth),
    agama = COALESCE(agama, religion),
    alamat = COALESCE(alamat, alamat_jalan),
    hp = COALESCE(hp, no_hp),
    nomor_kps = COALESCE(nomor_kps, no_kps),
    data_ayah = COALESCE(data_ayah, nama_ayah),
    data_ibu = COALESCE(data_ibu, nama_ibu);

-- Bidirectional trigger to keep both sets of column names synchronized
CREATE OR REPLACE FUNCTION sync_student_excel_columns()
RETURNS TRIGGER AS $$
BEGIN
    -- Sync nama <-> full_name
    IF NEW.nama IS NOT NULL AND (NEW.full_name IS NULL OR NEW.full_name = '') THEN
        NEW.full_name := NEW.nama;
    ELSIF NEW.full_name IS NOT NULL AND (NEW.nama IS NULL OR NEW.nama = '') THEN
        NEW.nama := NEW.full_name;
    END IF;

    -- Sync jk <-> gender
    IF NEW.jk IS NOT NULL AND (NEW.gender IS NULL OR NEW.gender = '') THEN
        NEW.gender := NEW.jk;
    ELSIF NEW.gender IS NOT NULL AND (NEW.jk IS NULL OR NEW.jk = '') THEN
        NEW.jk := NEW.gender;
    END IF;

    -- Sync tempat_lahir <-> place_of_birth
    IF NEW.tempat_lahir IS NOT NULL AND (NEW.place_of_birth IS NULL OR NEW.place_of_birth = '') THEN
        NEW.place_of_birth := NEW.tempat_lahir;
    ELSIF NEW.place_of_birth IS NOT NULL AND (NEW.tempat_lahir IS NULL OR NEW.tempat_lahir = '') THEN
        NEW.tempat_lahir := NEW.place_of_birth;
    END IF;

    -- Sync tanggal_lahir <-> date_of_birth
    IF NEW.tanggal_lahir IS NOT NULL AND NEW.date_of_birth IS NULL THEN
        NEW.date_of_birth := NEW.tanggal_lahir;
    ELSIF NEW.date_of_birth IS NOT NULL AND NEW.tanggal_lahir IS NULL THEN
        NEW.tanggal_lahir := NEW.date_of_birth;
    END IF;

    -- Sync agama <-> religion
    IF NEW.agama IS NOT NULL AND (NEW.religion IS NULL OR NEW.religion = '') THEN
        NEW.religion := NEW.agama;
    ELSIF NEW.religion IS NOT NULL AND (NEW.agama IS NULL OR NEW.agama = '') THEN
        NEW.agama := NEW.religion;
    END IF;

    -- Sync alamat <-> alamat_jalan
    IF NEW.alamat IS NOT NULL AND (NEW.alamat_jalan IS NULL OR NEW.alamat_jalan = '') THEN
        NEW.alamat_jalan := NEW.alamat;
    ELSIF NEW.alamat_jalan IS NOT NULL AND (NEW.alamat IS NULL OR NEW.alamat = '') THEN
        NEW.alamat := NEW.alamat_jalan;
    END IF;

    -- Sync hp <-> no_hp
    IF NEW.hp IS NOT NULL AND (NEW.no_hp IS NULL OR NEW.no_hp = '') THEN
        NEW.no_hp := NEW.hp;
    ELSIF NEW.no_hp IS NOT NULL AND (NEW.hp IS NULL OR NEW.hp = '') THEN
        NEW.hp := NEW.no_hp;
    END IF;

    -- Sync data_ayah <-> nama_ayah
    IF NEW.data_ayah IS NOT NULL AND (NEW.nama_ayah IS NULL OR NEW.nama_ayah = '') THEN
        NEW.nama_ayah := NEW.data_ayah;
    ELSIF NEW.nama_ayah IS NOT NULL AND (NEW.data_ayah IS NULL OR NEW.data_ayah = '') THEN
        NEW.data_ayah := NEW.nama_ayah;
    END IF;

    -- Sync data_ibu <-> nama_ibu
    IF NEW.data_ibu IS NOT NULL AND (NEW.nama_ibu IS NULL OR NEW.nama_ibu = '') THEN
        NEW.nama_ibu := NEW.data_ibu;
    ELSIF NEW.nama_ibu IS NOT NULL AND (NEW.data_ibu IS NULL OR NEW.data_ibu = '') THEN
        NEW.data_ibu := NEW.nama_ibu;
    END IF;

    -- Sync nomor_kps <-> no_kps
    IF NEW.nomor_kps IS NOT NULL AND (NEW.no_kps IS NULL OR NEW.no_kps = '') THEN
        NEW.no_kps := NEW.nomor_kps;
    ELSIF NEW.no_kps IS NOT NULL AND (NEW.nomor_kps IS NULL OR NEW.nomor_kps = '') THEN
        NEW.nomor_kps := NEW.no_kps;
    END IF;

    -- Sync rombel <-> rombel_saat_ini
    IF NEW.rombel IS NOT NULL AND (NEW.rombel_saat_ini IS NULL OR NEW.rombel_saat_ini = '') THEN
        NEW.rombel_saat_ini := NEW.rombel;
    ELSIF NEW.rombel_saat_ini IS NOT NULL AND (NEW.rombel IS NULL OR NEW.rombel = '') THEN
        NEW.rombel := NEW.rombel_saat_ini;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_sync_student_excel_columns ON students;
CREATE TRIGGER trigger_sync_student_excel_columns
BEFORE INSERT OR UPDATE ON students
FOR EACH ROW
EXECUTE FUNCTION sync_student_excel_columns();
