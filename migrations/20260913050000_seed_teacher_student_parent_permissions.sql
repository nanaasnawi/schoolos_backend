-- 20260913050000_seed_teacher_student_parent_permissions.sql
-- Seed core permissions for Guru, Siswa, and Wali Siswa to ensure Android and Web portals have full access

-- 1. Guru Permissions
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.perm
FROM roles r
CROSS JOIN (
    VALUES
    ('Student.Read'), ('Student.Create'), ('Student.Update'),
    ('Teacher.Read'),
    ('Guardian.Read'),
    ('Academic.Manage'),
    ('Learning.Curriculum.Read'),
    ('Learning.Syllabus.Read'),
    ('Learning.Material.Create'), ('Learning.Material.Read'), ('Learning.Material.Update'), ('Learning.Material.Delete'),
    ('Learning.Lesson.Create'), ('Learning.Lesson.Read'), ('Learning.Lesson.Update'), ('Learning.Lesson.Delete'),
    ('Learning.Session.Create'), ('Learning.Session.Read'), ('Learning.Session.Update'), ('Learning.Session.Delete'),
    ('Learning.Assignment.Create'), ('Learning.Assignment.Read'), ('Learning.Assignment.Update'), ('Learning.Assignment.Delete'),
    ('Learning.Quiz.Create'), ('Learning.Quiz.Read'), ('Learning.Quiz.Update'), ('Learning.Quiz.Delete'),
    ('Learning.Assessment.Configure'), ('Learning.Assessment.Read'),
    ('Learning.Progress.Read'), ('Learning.Progress.Update'),
    ('Learning.Achievement.Create'), ('Learning.Achievement.Read'), ('Learning.Achievement.Award'),
    ('Learning.Feed.Create'), ('Learning.Feed.Read'),
    ('Notification.Read'), ('Notification.Update'),
    ('Assessment.Input'), ('Assessment.Read'),
    ('Attendance.Record'), ('Attendance.Read'),
    ('School.Update')
) AS p(perm)
WHERE r.name IN ('Guru', 'Teacher')
ON CONFLICT DO NOTHING;

-- 2. Siswa Permissions
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.perm
FROM roles r
CROSS JOIN (
    VALUES
    ('Student.Read'),
    ('Teacher.Read'),
    ('Learning.Curriculum.Read'),
    ('Learning.Syllabus.Read'),
    ('Learning.Material.Read'),
    ('Learning.Lesson.Read'),
    ('Learning.Session.Read'),
    ('Learning.Assignment.Read'),
    ('Learning.Quiz.Read'),
    ('Learning.Assessment.Read'),
    ('Learning.Progress.Read'),
    ('Learning.Achievement.Read'),
    ('Learning.Feed.Read'),
    ('Notification.Read'), ('Notification.Update'),
    ('Assessment.Read'),
    ('Attendance.Read')
) AS p(perm)
WHERE r.name IN ('Siswa', 'Student')
ON CONFLICT DO NOTHING;

-- 3. Wali Siswa Permissions
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.perm
FROM roles r
CROSS JOIN (
    VALUES
    ('Student.Read'),
    ('Teacher.Read'),
    ('Learning.Session.Read'),
    ('Learning.Assignment.Read'),
    ('Learning.Progress.Read'),
    ('Learning.Achievement.Read'),
    ('Notification.Read'), ('Notification.Update'),
    ('Assessment.Read'),
    ('Attendance.Read')
) AS p(perm)
WHERE r.name IN ('Wali Siswa', 'Parent', 'Guardian')
ON CONFLICT DO NOTHING;
