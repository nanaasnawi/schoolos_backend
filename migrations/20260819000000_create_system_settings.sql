CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(100) PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO system_settings (key, value, updated_at) VALUES 
('maintenance', '{"maintenance_mode": false, "maintenance_message": "Sistem sedang dalam peningkatan performa server terjadwal. Silakan kembali dalam beberapa menit."}'::jsonb, NOW()),
('auth', '{"jwt_lifetime": "24", "refresh_token_lifetime": "7", "max_login_attempts": "5", "require_strong_password": true, "enforce_mfa_staff": false}'::jsonb, NOW()),
('dapodik', '{"default_ip": "127.0.0.1", "default_port": "5774", "timeout": "30", "auto_sync_daily": true}'::jsonb, NOW()),
('database', '{"pool_size": "50", "query_timeout": "5000", "auto_backup_daily": true, "retention_days": "90"}'::jsonb, NOW()),
('smtp', '{"host": "smtp.mailgun.org", "port": "587", "sender": "noreply@schoolos.id", "encryption": "TLS"}'::jsonb, NOW())
ON CONFLICT (key) DO NOTHING;
