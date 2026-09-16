-- 20260917020000_enhance_chat_persistence.sql
-- Support unread tracking, pagination indexing, client_message_id for idempotency

-- 1. Add client_message_id to inquiry_messages for offline idempotency
ALTER TABLE inquiry_messages ADD COLUMN IF NOT EXISTS client_message_id UUID;
CREATE UNIQUE INDEX IF NOT EXISTS idx_inquiry_messages_client_id 
    ON inquiry_messages(tenant_id, client_message_id) 
    WHERE client_message_id IS NOT NULL;

-- 2. Add message read receipts table
CREATE TABLE IF NOT EXISTS inquiry_message_reads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    thread_id UUID NOT NULL REFERENCES inquiry_threads(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES inquiry_messages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    read_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(message_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_inquiry_reads_thread_user ON inquiry_message_reads(thread_id, user_id);
CREATE INDEX IF NOT EXISTS idx_inquiry_reads_tenant ON inquiry_message_reads(tenant_id);

-- 3. Add last_read_at tracking on inquiry_threads per participant
ALTER TABLE inquiry_threads ADD COLUMN IF NOT EXISTS student_last_read_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
ALTER TABLE inquiry_threads ADD COLUMN IF NOT EXISTS teacher_last_read_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
