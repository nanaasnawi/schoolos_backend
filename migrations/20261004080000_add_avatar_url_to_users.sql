-- 20261004080000_add_avatar_url_to_users.sql
-- Add avatar_url column to users table for user profile photo persistence

ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_url TEXT;
