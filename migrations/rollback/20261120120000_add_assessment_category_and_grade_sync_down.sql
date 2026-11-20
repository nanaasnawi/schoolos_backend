-- Rollback for 20261120120000_add_assessment_category_and_grade_sync.sql
DROP INDEX IF EXISTS idx_ped_assess_source_unique;

ALTER TABLE gradebook_snapshots
    DROP COLUMN IF EXISTS ats_score;

ALTER TABLE pedagogical_assessments
    DROP COLUMN IF EXISTS source_type,
    DROP COLUMN IF EXISTS source_id;

ALTER TABLE quizzes
    DROP COLUMN IF EXISTS assessment_category;

ALTER TABLE assignments
    DROP COLUMN IF EXISTS assessment_category;
