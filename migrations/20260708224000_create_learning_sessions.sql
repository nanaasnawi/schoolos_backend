-- Create learning_sessions table
CREATE TABLE learning_sessions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lesson_id UUID NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    teacher_id UUID NOT NULL REFERENCES teachers(id) ON DELETE CASCADE,
    scheduled_at TIMESTAMP WITH TIME ZONE,
    started_at TIMESTAMP WITH TIME ZONE,
    ended_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20) NOT NULL DEFAULT 'scheduled',
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_learning_sessions_tenant_id ON learning_sessions(tenant_id);
CREATE INDEX idx_learning_sessions_lesson_id ON learning_sessions(lesson_id);
CREATE INDEX idx_learning_sessions_class_id ON learning_sessions(class_id);
CREATE INDEX idx_learning_sessions_teacher_id ON learning_sessions(teacher_id);
CREATE INDEX idx_learning_sessions_status ON learning_sessions(status);
CREATE INDEX idx_learning_sessions_deleted_at ON learning_sessions(deleted_at) WHERE deleted_at IS NULL;

-- Create session_attendances table
CREATE TABLE session_attendances (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'absent',
    checked_in_at TIMESTAMP WITH TIME ZONE,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(session_id, student_id)
);

CREATE INDEX idx_session_attendances_session_id ON session_attendances(session_id);
CREATE INDEX idx_session_attendances_student_id ON session_attendances(student_id);
CREATE INDEX idx_session_attendances_status ON session_attendances(status);

CREATE TRIGGER update_learning_sessions_updated_at
    BEFORE UPDATE ON learning_sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_session_attendances_updated_at
    BEFORE UPDATE ON session_attendances
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
