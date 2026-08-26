-- 0003_create_people_schema.sql

CREATE TABLE teachers (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    nip VARCHAR(50),
    full_name VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, nip)
);

CREATE INDEX idx_teachers_tenant_id ON teachers(tenant_id);
CREATE INDEX idx_teachers_user_id ON teachers(user_id);

CREATE TRIGGER update_teachers_updated_at
    BEFORE UPDATE ON teachers
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE guardians (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    full_name VARCHAR(255) NOT NULL,
    phone_number VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_guardians_tenant_id ON guardians(tenant_id);

CREATE TRIGGER update_guardians_updated_at
    BEFORE UPDATE ON guardians
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE students (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    guardian_id UUID REFERENCES guardians(id) ON DELETE SET NULL,
    nisn VARCHAR(50) NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, nisn)
);

CREATE INDEX idx_students_tenant_id ON students(tenant_id);
CREATE INDEX idx_students_guardian_id ON students(guardian_id);

CREATE TRIGGER update_students_updated_at
    BEFORE UPDATE ON students
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE staff (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    full_name VARCHAR(255) NOT NULL,
    job_title VARCHAR(100) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_staff_tenant_id ON staff(tenant_id);

CREATE TRIGGER update_staff_updated_at
    BEFORE UPDATE ON staff
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
