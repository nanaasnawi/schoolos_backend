-- 20260708203000_alter_student_status.sql

ALTER TABLE students
ADD COLUMN status VARCHAR(50) NOT NULL DEFAULT 'Active';

UPDATE students SET status = 'Active' WHERE is_active = true;
UPDATE students SET status = 'Inactive' WHERE is_active = false;

ALTER TABLE students DROP COLUMN is_active;
