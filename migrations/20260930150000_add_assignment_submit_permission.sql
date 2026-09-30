-- Migration: 20260930150000_add_assignment_submit_permission.sql
-- Description: Add granular Learning.Assignment.Submit permission so students can submit
-- assignments without needing the broader Update permission (which is for teachers only)

-- 1. Grant Learning.Assignment.Submit to Guru/Teacher (they can submit on behalf)
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'Learning.Assignment.Submit'
FROM roles r
WHERE r.name IN ('Guru', 'Teacher')
ON CONFLICT DO NOTHING;

-- 2. Grant Learning.Assignment.Submit to Siswa/Student (core action: submit own work)
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'Learning.Assignment.Submit'
FROM roles r
WHERE r.name IN ('Siswa', 'Student')
ON CONFLICT DO NOTHING;

-- 3. Grant to Admin/Operator/Kepsek (they have all permissions)
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'Learning.Assignment.Submit'
FROM roles r
WHERE r.name IN ('Admin', 'Operator', 'Kepala Sekolah', 'Administrator')
ON CONFLICT DO NOTHING;
