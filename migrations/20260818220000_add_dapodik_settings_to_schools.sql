-- 20260818220000_add_dapodik_settings_to_schools.sql
ALTER TABLE schools ADD COLUMN dapodik_url VARCHAR(255) DEFAULT 'http://localhost:5774';
ALTER TABLE schools ADD COLUMN dapodik_token VARCHAR(255);
