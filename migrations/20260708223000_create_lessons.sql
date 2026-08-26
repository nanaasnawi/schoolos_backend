-- Create lessons table
CREATE TABLE lessons (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    syllabus_id UUID NOT NULL REFERENCES syllabuses(id) ON DELETE CASCADE,
    code VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    learning_objectives TEXT,
    duration_minutes INTEGER NOT NULL DEFAULT 45,
    order_index INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, code)
);

CREATE INDEX idx_lessons_tenant_id ON lessons(tenant_id);
CREATE INDEX idx_lessons_syllabus_id ON lessons(syllabus_id);
CREATE INDEX idx_lessons_deleted_at ON lessons(deleted_at) WHERE deleted_at IS NULL;

-- Create lesson_plans table (RPP — detailed teaching plan)
CREATE TABLE lesson_plans (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lesson_id UUID NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
    teaching_methods TEXT,
    activities_opening TEXT,
    activities_core TEXT,
    activities_closing TEXT,
    resources TEXT,
    assessment_criteria TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(lesson_id)
);

CREATE INDEX idx_lesson_plans_lesson_id ON lesson_plans(lesson_id);
CREATE INDEX idx_lesson_plans_deleted_at ON lesson_plans(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_lessons_updated_at
    BEFORE UPDATE ON lessons
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_lesson_plans_updated_at
    BEFORE UPDATE ON lesson_plans
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
