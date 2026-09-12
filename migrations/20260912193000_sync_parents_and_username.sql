-- 20260912193000_sync_parents_and_username.sql
-- 1. Add username column to users table with unique index per tenant
ALTER TABLE users ADD COLUMN IF NOT EXISTS username VARCHAR(100);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_tenant_username ON users(tenant_id, LOWER(username)) WHERE username IS NOT NULL;

-- 2. Add complete parental columns to students table
ALTER TABLE students ADD COLUMN IF NOT EXISTS nama_ayah VARCHAR(255);
ALTER TABLE students ADD COLUMN IF NOT EXISTS nama_ibu VARCHAR(255);

-- 3. Ensure guardians table has relationship and address columns
ALTER TABLE guardians ADD COLUMN IF NOT EXISTS relationship VARCHAR(50);
ALTER TABLE guardians ADD COLUMN IF NOT EXISTS address TEXT;

-- 4. Add raw_token to user_qr_tokens to persist badge token safely without invalidating on reprint
ALTER TABLE user_qr_tokens ADD COLUMN IF NOT EXISTS raw_token VARCHAR(255);
