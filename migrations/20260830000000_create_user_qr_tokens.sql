-- 20260830000000_create_user_qr_tokens.sql
-- Table for Zero-Password QR Code / Physical Badge Token Authentication

CREATE TABLE IF NOT EXISTS user_qr_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,  -- SHA-256 hex string dari raw token
    token_type VARCHAR(20) NOT NULL DEFAULT 'BADGE', -- 'BADGE' (reusable kartu) | 'ONE_TIME' (magic login)
    label VARCHAR(100) DEFAULT 'Kartu Identitas Digital',
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMP WITH TIME ZONE,     -- NULL jika tidak pernah kadaluarsa
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_qr_tokens_hash ON user_qr_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_user_qr_tokens_user ON user_qr_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_user_qr_tokens_tenant ON user_qr_tokens(tenant_id);

CREATE TRIGGER update_user_qr_tokens_updated_at
    BEFORE UPDATE ON user_qr_tokens
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
