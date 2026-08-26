-- Create assignments table
CREATE TABLE assignments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lesson_id UUID NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    instructions TEXT,
    max_score INTEGER NOT NULL DEFAULT 100,
    due_at TIMESTAMP WITH TIME ZONE,
    assignment_type VARCHAR(20) NOT NULL DEFAULT 'individual',
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_assignments_tenant_id ON assignments(tenant_id);
CREATE INDEX idx_assignments_lesson_id ON assignments(lesson_id);
CREATE INDEX idx_assignments_status ON assignments(status);
CREATE INDEX idx_assignments_deleted_at ON assignments(deleted_at) WHERE deleted_at IS NULL;

-- Create assignment_submissions table
CREATE TABLE assignment_submissions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    assignment_id UUID NOT NULL REFERENCES assignments(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    content TEXT,
    file_url TEXT,
    submitted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'submitted',
    score INTEGER,
    feedback TEXT,
    graded_at TIMESTAMP WITH TIME ZONE,
    graded_by UUID,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(assignment_id, student_id)
);

CREATE INDEX idx_assignment_submissions_assignment_id ON assignment_submissions(assignment_id);
CREATE INDEX idx_assignment_submissions_student_id ON assignment_submissions(student_id);
CREATE INDEX idx_assignment_submissions_status ON assignment_submissions(status);

CREATE TRIGGER update_assignments_updated_at
    BEFORE UPDATE ON assignments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_assignment_submissions_updated_at
    BEFORE UPDATE ON assignment_submissions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
