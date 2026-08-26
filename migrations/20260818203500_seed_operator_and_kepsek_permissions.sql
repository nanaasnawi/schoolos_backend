-- 20260818203500_seed_operator_and_kepsek_permissions.sql

-- Grant all domain permissions to 'Operator/Staff' and 'Kepala Sekolah' roles across all tenants
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.perm
FROM roles r
CROSS JOIN (
    VALUES
    ('Student.Read'), ('Student.Create'), ('Student.Update'), ('Student.Delete'),
    ('Teacher.Read'), ('Teacher.Create'), ('Teacher.Update'), ('Teacher.Delete'),
    ('Guardian.Read'), ('Guardian.Create'), ('Guardian.Update'), ('Guardian.Delete'),
    ('Staff.Read'), ('Staff.Create'), ('Staff.Update'), ('Staff.Delete'),
    ('Academic.Manage'),
    ('Learning.Curriculum.Create'), ('Learning.Curriculum.Read'), ('Learning.Curriculum.Update'), ('Learning.Curriculum.Delete'),
    ('Learning.Syllabus.Create'), ('Learning.Syllabus.Read'), ('Learning.Syllabus.Update'), ('Learning.Syllabus.Delete'),
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
WHERE r.name IN ('Operator/Staff', 'Kepala Sekolah')
ON CONFLICT DO NOTHING;
