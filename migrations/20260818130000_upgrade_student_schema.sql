-- Migration to add detailed student profile columns mirroring Dapodik

ALTER TABLE students
ADD COLUMN IF NOT EXISTS nik VARCHAR(16),
ADD COLUMN IF NOT EXISTS gender VARCHAR(20),
ADD COLUMN IF NOT EXISTS place_of_birth VARCHAR(255),
ADD COLUMN IF NOT EXISTS date_of_birth DATE,
ADD COLUMN IF NOT EXISTS religion VARCHAR(50);
