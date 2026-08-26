-- Notification Engine Domain
-- Phase 2.12

CREATE TABLE notifications (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    title VARCHAR(255) NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    notification_type VARCHAR(100) NOT NULL,
    channel VARCHAR(20) NOT NULL DEFAULT 'in_app',
    reference_type VARCHAR(50),
    reference_id UUID,
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE notification_preferences (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    notification_type VARCHAR(100) NOT NULL,
    channels TEXT NOT NULL DEFAULT 'in_app',
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, user_id, notification_type)
);

CREATE INDEX idx_notifications_user ON notifications(user_id, created_at DESC);
CREATE INDEX idx_notifications_user_unread ON notifications(user_id, is_read) WHERE is_read = FALSE;
CREATE INDEX idx_notifications_tenant ON notifications(tenant_id);
CREATE INDEX idx_notification_prefs_user ON notification_preferences(user_id);
CREATE INDEX idx_notification_prefs_tenant ON notification_preferences(tenant_id);
