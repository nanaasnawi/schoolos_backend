-- Create student_progress table
CREATE TABLE student_progress (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    overall_progress NUMERIC(5,2) NOT NULL DEFAULT 0.00,
    lesson_completed INTEGER NOT NULL DEFAULT 0,
    lesson_total INTEGER NOT NULL DEFAULT 0,
    assignment_completed INTEGER NOT NULL DEFAULT 0,
    assignment_total INTEGER NOT NULL DEFAULT 0,
    quiz_completed INTEGER NOT NULL DEFAULT 0,
    quiz_total INTEGER NOT NULL DEFAULT 0,
    session_attended INTEGER NOT NULL DEFAULT 0,
    session_total INTEGER NOT NULL DEFAULT 0,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, class_id, subject_id)
);

CREATE INDEX idx_student_progress_tenant ON student_progress(tenant_id);
CREATE INDEX idx_student_progress_student ON student_progress(student_id);
CREATE INDEX idx_student_progress_class_subject ON student_progress(class_id, subject_id);

CREATE TRIGGER update_student_progress_updated_at
    BEFORE UPDATE ON student_progress
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
