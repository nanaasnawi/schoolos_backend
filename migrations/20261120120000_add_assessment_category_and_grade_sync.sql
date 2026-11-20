-- 20261120120000_add_assessment_category_and_grade_sync.sql
-- Automatic End-to-End Grading Synchronization:
-- 1. Add assessment_category on assignments & quizzes (FORMATIVE, SUMMATIVE_M1..M3, ATS, SAS)
-- 2. Add source linkage on pedagogical_assessments for idempotent upsert from Assignments & Quizzes
-- 3. Add ats_score to gradebook_snapshots so UTS/ATS & SAS/PAS reflect cleanly in e-Rapor

ALTER TABLE assignments
    ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';

ALTER TABLE quizzes
    ADD COLUMN IF NOT EXISTS assessment_category VARCHAR(32) NOT NULL DEFAULT 'FORMATIVE';

-- Backfill obvious legacy exam titles on quizzes so existing UTS/SAS exams map automatically
UPDATE quizzes
SET assessment_category = 'SAS'
WHERE assessment_category = 'FORMATIVE'
  AND title ~* '(^|[^A-Z0-9])(SAS|PAS|SUMATIF AKHIR|PENILAIAN AKHIR)([^A-Z0-9]|$)';

UPDATE quizzes
SET assessment_category = 'ATS'
WHERE assessment_category = 'FORMATIVE'
  AND title ~* '(^|[^A-Z0-9])(UTS|ATS|PTS|TENGAH SEMESTER)([^A-Z0-9]|$)';

ALTER TABLE pedagogical_assessments
    ADD COLUMN IF NOT EXISTS source_type VARCHAR(32) DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS source_id UUID DEFAULT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_ped_assess_source_unique
    ON pedagogical_assessments(tenant_id, source_type, source_id)
    WHERE source_id IS NOT NULL AND deleted_at IS NULL;

ALTER TABLE gradebook_snapshots
    ADD COLUMN IF NOT EXISTS ats_score NUMERIC(5,2) DEFAULT NULL;
