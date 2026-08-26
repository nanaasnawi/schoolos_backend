-- Create learning_materials table
CREATE TABLE learning_materials (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lesson_id UUID,
    material_type VARCHAR(20) NOT NULL DEFAULT 'Document',
    title VARCHAR(255) NOT NULL,
    description TEXT,
    storage_key TEXT,
    external_url TEXT,
    order_index INTEGER NOT NULL DEFAULT 0,
    visibility VARCHAR(20) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_learning_materials_tenant_id ON learning_materials(tenant_id);
CREATE INDEX idx_learning_materials_lesson_id ON learning_materials(lesson_id);
CREATE INDEX idx_learning_materials_deleted_at ON learning_materials(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_learning_materials_updated_at
    BEFORE UPDATE ON learning_materials
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
