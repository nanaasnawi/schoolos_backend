-- Create syllabuses table
CREATE TABLE syllabuses (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    curriculum_id UUID NOT NULL REFERENCES curricula(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
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

CREATE INDEX idx_syllabuses_tenant_id ON syllabuses(tenant_id);
CREATE INDEX idx_syllabuses_curriculum_id ON syllabuses(curriculum_id);
CREATE INDEX idx_syllabuses_subject_id ON syllabuses(subject_id);
CREATE INDEX idx_syllabuses_grade_level_id ON syllabuses(grade_level_id);
CREATE INDEX idx_syllabuses_deleted_at ON syllabuses(deleted_at) WHERE deleted_at IS NULL;

-- Create syllabus_competencies table
CREATE TABLE syllabus_competencies (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    syllabus_id UUID NOT NULL REFERENCES syllabuses(id) ON DELETE CASCADE,
    code VARCHAR(50) NOT NULL,
    competency_type VARCHAR(20) NOT NULL DEFAULT 'Knowledge',
    description TEXT NOT NULL,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(syllabus_id, code)
);

CREATE INDEX idx_syllabus_competencies_syllabus_id ON syllabus_competencies(syllabus_id);
CREATE INDEX idx_syllabus_competencies_deleted_at ON syllabus_competencies(deleted_at) WHERE deleted_at IS NULL;

CREATE TRIGGER update_syllabuses_updated_at
    BEFORE UPDATE ON syllabuses
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_syllabus_competencies_updated_at
    BEFORE UPDATE ON syllabus_competencies
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
