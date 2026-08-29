-- seed_master_data.sql
-- Seed script for Real Data Implementation (Phase 3)
-- This creates 2 Tenants, Users, Academic Years, Grade Levels, Teachers, Classes, Students, and Enrollments.

BEGIN;

-- =========================================================================
-- 1. TENANTS & ROLES
-- =========================================================================
INSERT INTO tenants (id, name, domain, is_active, created_at, updated_at, npsn) VALUES 
('11111111-1111-1111-1111-111111111111', 'Sekolah Nusantara Jaya', 'nusantara.sch.id', true, NOW(), NOW(), '20299999'),
('11111111-2222-2222-2222-111111111111', 'Sekolah Harapan Bangsa', 'harapan.sch.id', true, NOW(), NOW(), '20388888')
ON CONFLICT (id) DO NOTHING;

INSERT INTO roles (id, tenant_id, name, permissions, created_at) VALUES 
('22222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111', 'Admin', '["TenantView", "TenantManage", "SchoolView", "SchoolManage", "StudentView", "StudentCreate", "StudentManage", "TeacherView", "TeacherCreate", "TeacherManage", "ClassView", "ClassCreate", "ClassManage"]', NOW()),
('22222222-2222-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', 'Admin', '["TenantView", "TenantManage", "SchoolView", "SchoolManage", "StudentView", "StudentCreate", "StudentManage", "TeacherView", "TeacherCreate", "TeacherManage", "ClassView", "ClassCreate", "ClassManage"]', NOW())
ON CONFLICT (id) DO NOTHING;

-- 'secretpassword'
INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at) VALUES 
('33333333-3333-3333-3333-333333333333', '11111111-1111-1111-1111-111111111111', 'admin@nusantarajaya.id', '$argon2id$v=19$m=19456,t=2,p=1$A08/422yL+lT+Q+O/6QYVw$rWkPq6MvS28eKz1V3Yq0oJ/tQv5Z+u+13Xz/2s4zXoM', 'Admin Nusantara', true, NOW(), NOW()),
('33333333-2222-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', 'admin@harapanbangsa.id', '$argon2id$v=19$m=19456,t=2,p=1$A08/422yL+lT+Q+O/6QYVw$rWkPq6MvS28eKz1V3Yq0oJ/tQv5Z+u+13Xz/2s4zXoM', 'Admin Harapan', true, NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

INSERT INTO user_roles (user_id, role_id) VALUES 
('33333333-3333-3333-3333-333333333333', '22222222-2222-2222-2222-222222222222'),
('33333333-2222-2222-2222-111111111111', '22222222-2222-2222-2222-111111111111')
ON CONFLICT DO NOTHING;

-- =========================================================================
-- 2. ACADEMIC YEARS & GRADE LEVELS
-- =========================================================================
INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at) VALUES 
('44444444-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '2024/2025 Semester 1', '2024-07-01', '2024-12-31', false, NOW(), NOW()),
('44444444-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '2024/2025 Semester 2', '2025-01-01', '2025-06-30', true, NOW(), NOW()),
('44444444-1111-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '2024/2025 Semester 1', '2024-07-01', '2024-12-31', false, NOW(), NOW()),
('44444444-2222-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '2024/2025 Semester 2', '2025-01-01', '2025-06-30', true, NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

INSERT INTO grade_levels (id, tenant_id, level, name, created_at, updated_at) VALUES 
('55555555-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', 1, 'Kelas 1', NOW(), NOW()),
('55555555-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', 2, 'Kelas 2', NOW(), NOW()),
('55555555-1111-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', 4, 'Kelas 4', NOW(), NOW()),
('55555555-2222-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', 5, 'Kelas 5', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- =========================================================================
-- 3. TEACHERS & CLASSES
-- =========================================================================
INSERT INTO teachers (id, tenant_id, nip, full_name, is_active, created_at, updated_at) VALUES
('66666666-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '198001012010011001', 'Budi Santoso, S.Pd', true, NOW(), NOW()),
('66666666-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '198202022010022002', 'Siti Aminah, M.Pd', true, NOW(), NOW()),
('66666666-3333-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '197505052005011005', 'Agus Supriyanto, S.Pd', true, NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name, capacity, created_at, updated_at) VALUES
('77777777-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '44444444-2222-1111-1111-111111111111', '55555555-1111-1111-1111-111111111111', '66666666-1111-1111-1111-111111111111', 'Kelas 1-A', 30, NOW(), NOW()),
('77777777-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '44444444-2222-1111-1111-111111111111', '55555555-1111-1111-1111-111111111111', '66666666-2222-1111-1111-111111111111', 'Kelas 1-B', 30, NOW(), NOW()),
('77777777-3333-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '44444444-2222-2222-2222-111111111111', '55555555-1111-2222-2222-111111111111', '66666666-3333-2222-2222-111111111111', 'Kelas 4-A', 30, NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- =========================================================================
-- 4. STUDENTS & ENROLLMENTS
-- =========================================================================
INSERT INTO students (id, tenant_id, nisn, full_name, status, created_at, updated_at) VALUES
('88888888-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '0081234501', 'Ahmad Fauzi', 'Active', NOW(), NOW()),
('88888888-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '0081234502', 'Budi Hermawan', 'Active', NOW(), NOW()),
('88888888-3333-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '0081234503', 'Citra Kirana', 'Active', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

INSERT INTO enrollments (id, tenant_id, student_id, class_id, academic_year_id, status, enrolled_at) VALUES
('99999999-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '88888888-1111-1111-1111-111111111111', '77777777-1111-1111-1111-111111111111', '44444444-2222-1111-1111-111111111111', 'Active', NOW()),
('99999999-2222-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', '88888888-2222-1111-1111-111111111111', '77777777-1111-1111-1111-111111111111', '44444444-2222-1111-1111-111111111111', 'Active', NOW()),
('99999999-3333-2222-2222-111111111111', '11111111-2222-2222-2222-111111111111', '88888888-3333-2222-2222-111111111111', '77777777-3333-2222-2222-111111111111', '44444444-2222-2222-2222-111111111111', 'Active', NOW())
ON CONFLICT (id) DO NOTHING;

COMMIT;
