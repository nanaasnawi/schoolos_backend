CREATE TABLE IF NOT EXISTS idempotency_keys (
    idempotency_key VARCHAR(255) PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_id UUID,
    request_hash VARCHAR(255) NOT NULL,
    response_status INT NOT NULL,
    response_body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_idempotency_keys_tenant_id ON idempotency_keys(tenant_id);
