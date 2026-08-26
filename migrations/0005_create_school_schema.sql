-- 0005_create_school_schema.sql

CREATE TABLE schools (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    npsn VARCHAR(50),
    address TEXT,
    phone_number VARCHAR(50),
    email VARCHAR(255),
    logo_url VARCHAR(255),
    status VARCHAR(50) NOT NULL DEFAULT 'Active',
    accreditation VARCHAR(10),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id)
);

CREATE INDEX idx_schools_tenant_id ON schools(tenant_id);

CREATE TRIGGER update_schools_updated_at
    BEFORE UPDATE ON schools
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
