-- 20260708203500_create_audit_logs.sql

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    school_id UUID,
    request_id VARCHAR(255),
    actor_id UUID,
    action VARCHAR(255) NOT NULL,
    resource VARCHAR(255),
    permission VARCHAR(255),
    policy VARCHAR(255),
    decision VARCHAR(50) NOT NULL,
    reason TEXT,
    ip VARCHAR(45),
    user_agent TEXT,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_tenant_id ON audit_logs(tenant_id);
CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp);
CREATE INDEX idx_audit_logs_request_id ON audit_logs(request_id);
