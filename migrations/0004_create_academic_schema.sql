-- 0004_create_academic_schema.sql

CREATE TABLE academic_years (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(50) NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_academic_years_tenant_id ON academic_years(tenant_id);

CREATE TRIGGER update_academic_years_updated_at
    BEFORE UPDATE ON academic_years
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE terms (
    id UUID PRIMARY KEY,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    name VARCHAR(50) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_terms_academic_year_id ON terms(academic_year_id);

CREATE TRIGGER update_terms_updated_at
    BEFORE UPDATE ON terms
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE grade_levels (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    level INTEGER NOT NULL,
    name VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, level)
);

CREATE INDEX idx_grade_levels_tenant_id ON grade_levels(tenant_id);

CREATE TRIGGER update_grade_levels_updated_at
    BEFORE UPDATE ON grade_levels
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE classes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE CASCADE,
    homeroom_teacher_id UUID REFERENCES teachers(id) ON DELETE RESTRICT, -- Cannot delete teacher if assigned
    name VARCHAR(50) NOT NULL,
    capacity INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_classes_tenant_id ON classes(tenant_id);
CREATE INDEX idx_classes_academic_year_id ON classes(academic_year_id);
CREATE INDEX idx_classes_homeroom_teacher_id ON classes(homeroom_teacher_id);

CREATE TRIGGER update_classes_updated_at
    BEFORE UPDATE ON classes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE subjects (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, code)
);

CREATE INDEX idx_subjects_tenant_id ON subjects(tenant_id);

CREATE TRIGGER update_subjects_updated_at
    BEFORE UPDATE ON subjects
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE enrollments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'Active', -- Active, Closed
    enrolled_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_unique_active_enrollment ON enrollments(student_id, academic_year_id) WHERE status = 'Active';

CREATE INDEX idx_enrollments_tenant_id ON enrollments(tenant_id);
CREATE INDEX idx_enrollments_student_id ON enrollments(student_id);
CREATE INDEX idx_enrollments_class_id ON enrollments(class_id);
