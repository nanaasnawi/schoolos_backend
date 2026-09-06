-- Announcements Table for SchoolOS
-- Non-Firebase Push Notifications & Information Board

CREATE TABLE IF NOT EXISTS announcements (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    title VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    category VARCHAR(50) NOT NULL DEFAULT 'AKADEMIK',
    target VARCHAR(100) NOT NULL DEFAULT 'Semua Siswa & Guru',
    author VARCHAR(255) NOT NULL,
    is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
    push_status BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_announcements_tenant ON announcements(tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_announcements_pinned ON announcements(tenant_id, is_pinned DESC, created_at DESC);
