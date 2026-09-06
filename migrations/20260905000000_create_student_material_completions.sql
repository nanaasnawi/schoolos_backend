-- Create student_material_completions table
CREATE TABLE IF NOT EXISTS student_material_completions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    material_id UUID NOT NULL REFERENCES learning_materials(id) ON DELETE CASCADE,
    completed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, material_id)
);

CREATE INDEX IF NOT EXISTS idx_student_material_completions_tenant ON student_material_completions(tenant_id);
CREATE INDEX IF NOT EXISTS idx_student_material_completions_student ON student_material_completions(student_id);
CREATE INDEX IF NOT EXISTS idx_student_material_completions_material ON student_material_completions(material_id);
