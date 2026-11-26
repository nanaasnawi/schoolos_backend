-- 20261126000000_ensure_educator_role_permissions.sql
-- Ensure all non-admin Educator roles (Guru, Teacher, Tutor, Pamong, Pendidik, Pengajar, Wali Kelas)
-- across all tenants retain required teaching/workstation read & instructional permissions
-- without granting tenant/admin management permissions.

INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.perm
FROM roles r
CROSS JOIN (
    VALUES
    ('Student.Read'),
    ('Student.Update'),
    ('Teacher.Read'),
    ('Guardian.Read'),
    ('Learning.Curriculum.Read'),
    ('Learning.Syllabus.Read'),
    ('Learning.Material.Create'),
    ('Learning.Material.Read'),
    ('Learning.Material.Update'),
    ('Learning.Material.Delete'),
    ('Learning.Lesson.Create'),
    ('Learning.Lesson.Read'),
    ('Learning.Lesson.Update'),
    ('Learning.Lesson.Delete'),
    ('Learning.Session.Create'),
    ('Learning.Session.Read'),
    ('Learning.Session.Update'),
    ('Learning.Session.Delete'),
    ('Learning.Assignment.Create'),
    ('Learning.Assignment.Read'),
    ('Learning.Assignment.Update'),
    ('Learning.Assignment.Delete'),
    ('Learning.Quiz.Create'),
    ('Learning.Quiz.Read'),
    ('Learning.Quiz.Update'),
    ('Learning.Quiz.Delete'),
    ('Learning.Assessment.Configure'),
    ('Learning.Assessment.Read'),
    ('Learning.Progress.Read'),
    ('Learning.Progress.Update'),
    ('Learning.Achievement.Create'),
    ('Learning.Achievement.Read'),
    ('Learning.Achievement.Award'),
    ('Learning.Feed.Create'),
    ('Learning.Feed.Read'),
    ('Notification.Read'),
    ('Notification.Update'),
    ('Assessment.Input'),
    ('Assessment.Read'),
    ('Attendance.Record'),
    ('Attendance.Read')
) AS p(perm)
WHERE (
    LOWER(r.name) LIKE '%guru%'
    OR LOWER(r.name) LIKE '%teacher%'
    OR LOWER(r.name) LIKE '%tutor%'
    OR LOWER(r.name) LIKE '%pamong%'
    OR LOWER(r.name) LIKE '%pendidik%'
    OR LOWER(r.name) LIKE '%pengajar%'
    OR LOWER(r.name) LIKE '%wali kelas%'
)
  AND LOWER(r.name) NOT LIKE '%admin%'
  AND LOWER(r.name) NOT LIKE '%operator%'
  AND LOWER(r.name) NOT LIKE '%kepala%'
  AND LOWER(r.name) NOT LIKE '%principal%'
  AND LOWER(r.name) NOT LIKE '%staff%'
  AND LOWER(r.name) NOT LIKE '%bendahara%'
  AND LOWER(r.name) NOT LIKE '%siswa%'
  AND LOWER(r.name) NOT LIKE '%student%'
ON CONFLICT DO NOTHING;
