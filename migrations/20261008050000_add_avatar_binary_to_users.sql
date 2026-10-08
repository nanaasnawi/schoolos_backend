-- 20261008050000_add_avatar_binary_to_users.sql
-- Store profile avatar photo binary and MIME directly in PostgreSQL database, avoiding local filesystem storage or caching

ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_data BYTEA;
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_mime VARCHAR(50);
