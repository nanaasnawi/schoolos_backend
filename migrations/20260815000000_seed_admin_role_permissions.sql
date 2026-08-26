-- 20260815000000_seed_admin_role_permissions.sql

-- 1. Ensure roles table schema matches domain Role struct
ALTER TABLE roles ADD COLUMN IF NOT EXISTS is_system_default BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE roles ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();

-- 2. Ensure default main tenant exists
INSERT INTO tenants (id, name, domain, is_active, created_at, updated_at)
VALUES ('00000000-0000-0000-0000-000000000001', 'School OS Main Tenant', 'schoolos.id', true, NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- 3. Ensure Admin role exists for all tenants
INSERT INTO roles (id, tenant_id, name, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), id, 'Admin', true, NOW(), NOW()
FROM tenants t
WHERE NOT EXISTS (SELECT 1 FROM roles r WHERE r.tenant_id = t.id AND r.name = 'Admin');

-- 4. Assign all users without a role to their tenant's Admin role
INSERT INTO user_roles (user_id, role_id)
SELECT u.id, r.id
FROM users u
JOIN roles r ON r.tenant_id = u.tenant_id AND r.name = 'Admin'
ON CONFLICT DO NOTHING;

-- 5. Grant all domain permissions to Admin roles across all tenants
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
    ('School.Update'), ('Tenant.Manage'), ('System.Internal')
) AS p(perm)
WHERE r.name = 'Admin'
ON CONFLICT DO NOTHING;
