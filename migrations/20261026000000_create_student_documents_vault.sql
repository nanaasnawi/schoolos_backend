-- 20261026000000_create_student_documents_vault.sql
-- Description: Create student_documents vault table for Digital Student Dossier & Verification Engine
-- Complies with ADR-0004 (Multi-Tenant RLS), ADR-0011 (Backend-Centric Logic), & ADR-0020 (Digital Student Dossier)

-- 1. Ensure helper function for tenant isolation exists
CREATE OR REPLACE FUNCTION is_tenant_allowed(record_tenant_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    curr_tenant TEXT;
    bypass TEXT;
    as_salafiyah_id UUID;
BEGIN
    bypass := current_setting('app.bypass_rls', true);
    IF bypass = 'on' THEN
        RETURN true;
    END IF;

    curr_tenant := NULLIF(current_setting('app.current_tenant_id', true), '');
    IF curr_tenant IS NOT NULL THEN
        RETURN record_tenant_id = curr_tenant::uuid;
    END IF;

    SELECT id INTO as_salafiyah_id FROM tenants WHERE name ILIKE '%As-Salafiyah%' LIMIT 1;
    IF as_salafiyah_id IS NOT NULL AND record_tenant_id = as_salafiyah_id THEN
        RETURN true;
    END IF;

    RETURN false;
END;
$$ LANGUAGE plpgsql STABLE SECURITY DEFINER;

-- 2. Create student_documents table
CREATE TABLE IF NOT EXISTS student_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    document_type VARCHAR(50) NOT NULL, -- 'KK', 'AKTA', 'IJAZAH_SD', 'IJAZAH_SMP', 'KTP', 'PAS_FOTO', 'KIP'
    file_url TEXT NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    file_size_bytes BIGINT NOT NULL DEFAULT 0,
    mime_type VARCHAR(100) NOT NULL, -- e.g. 'image/jpeg', 'image/png', 'image/webp', 'application/pdf'
    verification_status VARCHAR(30) NOT NULL DEFAULT 'PENDING', -- 'PENDING', 'VERIFIED', 'REJECTED'
    verified_by UUID REFERENCES users(id) ON DELETE SET NULL,
    verified_at TIMESTAMP WITH TIME ZONE,
    rejection_reason TEXT,
    notes TEXT,
    is_optional BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, student_id, document_type)
);

-- 3. High-performance lookup indices
CREATE INDEX IF NOT EXISTS idx_student_docs_tenant_student ON student_documents(tenant_id, student_id);
CREATE INDEX IF NOT EXISTS idx_student_docs_tenant_status ON student_documents(tenant_id, verification_status);
CREATE INDEX IF NOT EXISTS idx_student_docs_tenant_type ON student_documents(tenant_id, document_type);

-- 4. Auto-update timestamp trigger
DROP TRIGGER IF EXISTS update_student_documents_updated_at ON student_documents;
CREATE TRIGGER update_student_documents_updated_at
    BEFORE UPDATE ON student_documents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- 5. Enforce Multi-Tenant Row Level Security (RLS)
ALTER TABLE student_documents ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS student_documents_tenant_isolation ON student_documents;
CREATE POLICY student_documents_tenant_isolation ON student_documents
    FOR ALL
    USING (is_tenant_allowed(tenant_id))
    WITH CHECK (is_tenant_allowed(tenant_id));
