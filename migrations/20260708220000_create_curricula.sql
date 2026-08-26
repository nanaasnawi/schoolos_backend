-- Create curricula table for curriculum management
CREATE TABLE curricula (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, code)
);

CREATE INDEX idx_curricula_tenant_id ON curricula(tenant_id);
CREATE INDEX idx_curricula_deleted_at ON curricula(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_curricula_updated_at
    BEFORE UPDATE ON curricula
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
