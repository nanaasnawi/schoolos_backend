-- 20261010210000_create_kurikulum_merdeka_assessments_down.sql
-- Rollback for Kurikulum Merdeka pedagogical assessments

DROP TRIGGER IF EXISTS update_assessment_student_results_updated_at ON assessment_student_results;
DROP TABLE IF EXISTS assessment_student_results CASCADE;

DROP TRIGGER IF EXISTS update_pedagogical_assessments_updated_at ON pedagogical_assessments;
DROP TABLE IF EXISTS pedagogical_assessments CASCADE;
