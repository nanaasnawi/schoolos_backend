-- Rollback 20261125020000_add_multitenant_hot_indexes_and_analytics_projection.sql

DROP INDEX IF EXISTS idx_idempotency_keys_created_at_asc;
DROP INDEX IF EXISTS idx_audit_logs_tenant_ts_desc;
DROP INDEX IF EXISTS idx_notifications_tenant_user_read_created;
DROP INDEX IF EXISTS idx_gradebook_snapshots_tenant_yr_sem;
DROP INDEX IF EXISTS idx_gradebook_entries_tenant_class_subj_student;
DROP INDEX IF EXISTS idx_learning_materials_tenant_class_deleted;
DROP INDEX IF EXISTS idx_assignment_submissions_tenant_asg_student;
DROP INDEX IF EXISTS idx_assignments_tenant_class_deleted;
DROP INDEX IF EXISTS idx_quiz_attempts_tenant_quiz_student;
DROP INDEX IF EXISTS idx_enrollments_tenant_student_status;
DROP INDEX IF EXISTS idx_enrollments_tenant_class_status;
DROP INDEX IF EXISTS idx_guardians_tenant_user;
DROP INDEX IF EXISTS idx_teachers_tenant_user_deleted;
DROP INDEX IF EXISTS idx_students_tenant_active_deleted;

DROP TABLE IF EXISTS analytics_school_overview;
DROP TABLE IF EXISTS quiz_proctor_events;
DROP TABLE IF EXISTS quiz_executive_pre_start_batches;
DROP TABLE IF EXISTS quiz_pre_start_notices;
