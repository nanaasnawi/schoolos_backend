-- Migration: Create Dapodik Sync Records, Outbox Jobs, and Opaque Onboarding Tokens

CREATE TABLE IF NOT EXISTS dapodik_sync_records (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    nisn VARCHAR(10) NOT NULL,
    nik VARCHAR(16) NOT NULL,
    nama_school_os VARCHAR(255) NOT NULL,
    nama_dapodik VARCHAR(255) NOT NULL,
    rombel VARCHAR(50) NOT NULL,
    identity_state VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
    mobility_case VARCHAR(50) NOT NULL DEFAULT 'NONE',
    classification VARCHAR(50) NOT NULL DEFAULT 'MATCH',
    action_recommended TEXT NOT NULL DEFAULT '',
    stage VARCHAR(50) NOT NULL DEFAULT 'VERIFIED',
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_dapodik_sync_tenant ON dapodik_sync_records(tenant_id);
CREATE INDEX IF NOT EXISTS idx_dapodik_sync_nisn ON dapodik_sync_records(nisn);

CREATE TABLE IF NOT EXISTS local_bridge_outbox_jobs (
    job_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    req_id VARCHAR(100) NOT NULL,
    operation VARCHAR(100) NOT NULL,
    entity_id VARCHAR(100) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    attempts INT NOT NULL DEFAULT 0,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING_RETRY',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_outbox_tenant ON local_bridge_outbox_jobs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_outbox_idempotency ON local_bridge_outbox_jobs(idempotency_key);

CREATE TABLE IF NOT EXISTS onboarding_tokens (
    request_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    opaque_token VARCHAR(255) NOT NULL UNIQUE,
    nonce VARCHAR(255) NOT NULL,
    student_data JSONB NOT NULL,
    token_state VARCHAR(50) NOT NULL DEFAULT 'ISSUED',
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_onboarding_token ON onboarding_tokens(opaque_token);
