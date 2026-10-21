-- 20261021110000_fix_rls_dynamic_multitenant_and_student_mutations_down.sql
-- Description: Rollback script for 20261021110000_fix_rls_dynamic_multitenant_and_student_mutations.sql

DROP POLICY IF EXISTS student_mutations_tenant_isolation ON student_mutations;
DROP TABLE IF EXISTS student_mutations;

ALTER TABLE students 
    DROP COLUMN IF EXISTS exit_category,
    DROP COLUMN IF EXISTS exit_date,
    DROP COLUMN IF EXISTS target_school,
    DROP COLUMN IF EXISTS exit_reference_no,
    DROP COLUMN IF EXISTS exit_notes,
    DROP COLUMN IF EXISTS last_rombel;
