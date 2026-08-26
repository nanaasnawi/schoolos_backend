-- 20260817220000_seed_standard_user_roles.sql
-- Migration to introduce standard system roles with platform scope (Web / Android)

ALTER TABLE roles ADD COLUMN IF NOT EXISTS description VARCHAR(255);
ALTER TABLE roles ADD COLUMN IF NOT EXISTS allowed_platforms VARCHAR(100) NOT NULL DEFAULT 'WEB, ANDROID';

-- Insert / Update standard roles for all tenants
INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Kepala Sekolah', 'Kepala Sekolah / Pimpinan Unit', 'WEB, ANDROID', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();

INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Guru', 'Guru Pengajar & Wali Kelas', 'WEB, ANDROID', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();

INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Operator/Staff', 'Operator Sekolah & Staf Administrasi', 'WEB', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();

INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Bendahara', 'Bendahara & Pengelola Keuangan Sekolah', 'WEB, ANDROID', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();

INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Siswa', 'Peserta Didik / Siswa', 'ANDROID', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();

INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at)
SELECT gen_random_uuid(), t.id, 'Wali Siswa', 'Orang Tua / Wali Murid', 'ANDROID', true, NOW(), NOW()
FROM tenants t
ON CONFLICT (tenant_id, name) DO UPDATE 
SET description = EXCLUDED.description, allowed_platforms = EXCLUDED.allowed_platforms, updated_at = NOW();
