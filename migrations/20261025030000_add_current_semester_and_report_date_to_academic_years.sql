-- Migration: 20261025030000_add_current_semester_and_report_date_to_academic_years.sql
-- Description: Add current_semester (1=Gasal, 2=Genap) and report_date (tanggal pembagian rapor) to academic_years table for SSOT

ALTER TABLE academic_years 
    ADD COLUMN IF NOT EXISTS current_semester SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS report_date DATE;

-- Set default values for active academic years
UPDATE academic_years 
SET current_semester = 1,
    report_date = '2026-12-19'
WHERE name = '2026/2027';
