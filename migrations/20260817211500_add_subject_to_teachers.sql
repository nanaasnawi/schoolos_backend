-- Add subject column to teachers table
ALTER TABLE teachers ADD COLUMN IF NOT EXISTS subject VARCHAR(100);
