-- 20260917040000_create_notification_dedup_and_queue.sql
-- Support notification deduplication key, quiet hours scheduling, and parent critical priorities

-- 1. Add deduplication, quiet hours scheduling, and priority to notifications
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS dedup_key VARCHAR(255);
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS scheduled_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS is_urgent BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS priority VARCHAR(20) NOT NULL DEFAULT 'NORMAL'; -- 'CRITICAL' or 'NORMAL'

CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_tenant_dedup 
    ON notifications(tenant_id, dedup_key) 
    WHERE dedup_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_notifications_scheduled 
    ON notifications(tenant_id, user_id, scheduled_at, is_read);
