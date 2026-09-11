-- Migration: Widen dapodik_sync_records and students columns to prevent transaction aborts on long or missing identifiers
ALTER TABLE dapodik_sync_records ALTER COLUMN nisn TYPE VARCHAR(50);
ALTER TABLE dapodik_sync_records ALTER COLUMN nik TYPE VARCHAR(50);
ALTER TABLE dapodik_sync_records ALTER COLUMN nik DROP NOT NULL;
ALTER TABLE dapodik_sync_records ALTER COLUMN rombel TYPE VARCHAR(100);

ALTER TABLE students ALTER COLUMN nik TYPE VARCHAR(50);
