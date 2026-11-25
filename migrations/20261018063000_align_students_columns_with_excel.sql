-- 20261018063000_align_students_columns_with_excel.sql
-- Add rombel column to students and remove redundant duplicate columns

ALTER TABLE students ADD COLUMN IF NOT EXISTS rombel VARCHAR(150);

-- Drop duplicate columns if they exist
ALTER TABLE students
    DROP COLUMN IF EXISTS nama,
    DROP COLUMN IF EXISTS jk,
    DROP COLUMN IF EXISTS tempat_lahir,
    DROP COLUMN IF EXISTS tanggal_lahir,
    DROP COLUMN IF EXISTS agama,
    DROP COLUMN IF EXISTS alamat,
    DROP COLUMN IF EXISTS hp,
    DROP COLUMN IF EXISTS nomor_kps,
    DROP COLUMN IF EXISTS data_ayah,
    DROP COLUMN IF EXISTS data_ibu,
    DROP COLUMN IF EXISTS rombel_saat_ini;

DROP TRIGGER IF EXISTS trigger_sync_student_excel_columns ON students;
DROP FUNCTION IF EXISTS sync_student_excel_columns();
