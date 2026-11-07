-- Relax rigid foreign keys on inquiry_threads to allow inquiries between:
-- 1. Students <-> Teachers
-- 2. Students <-> Kepala Sekolah (Principal)
-- 3. Kepala Sekolah <-> Dewan Guru (Teachers)
ALTER TABLE inquiry_threads DROP CONSTRAINT IF EXISTS inquiry_threads_student_id_fkey;
ALTER TABLE inquiry_threads DROP CONSTRAINT IF EXISTS inquiry_threads_teacher_id_fkey;

