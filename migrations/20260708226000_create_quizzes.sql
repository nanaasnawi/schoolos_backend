-- Create quizzes table
CREATE TABLE quizzes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    lesson_id UUID NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    time_limit_minutes INTEGER,
    passing_score INTEGER NOT NULL DEFAULT 0,
    max_score INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    questions_count INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID
);

CREATE INDEX idx_quizzes_tenant_id ON quizzes(tenant_id);
CREATE INDEX idx_quizzes_lesson_id ON quizzes(lesson_id);
CREATE INDEX idx_quizzes_status ON quizzes(status);
CREATE INDEX idx_quizzes_deleted_at ON quizzes(deleted_at) WHERE deleted_at IS NULL;

-- Create quiz_questions table
CREATE TABLE quiz_questions (
    id UUID PRIMARY KEY,
    quiz_id UUID NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
    question_text TEXT NOT NULL,
    question_type VARCHAR(20) NOT NULL DEFAULT 'multiple_choice',
    points INTEGER NOT NULL DEFAULT 1,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quiz_questions_quiz_id ON quiz_questions(quiz_id);

-- Create quiz_choices table
CREATE TABLE quiz_choices (
    id UUID PRIMARY KEY,
    question_id UUID NOT NULL REFERENCES quiz_questions(id) ON DELETE CASCADE,
    choice_text TEXT NOT NULL,
    is_correct BOOLEAN NOT NULL DEFAULT false,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quiz_choices_question_id ON quiz_choices(question_id);

-- Create quiz_attempts table
CREATE TABLE quiz_attempts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    quiz_id UUID NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE,
    score INTEGER,
    total_points INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'in_progress',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(quiz_id, student_id)
);

CREATE INDEX idx_quiz_attempts_quiz_id ON quiz_attempts(quiz_id);
CREATE INDEX idx_quiz_attempts_student_id ON quiz_attempts(student_id);
CREATE INDEX idx_quiz_attempts_status ON quiz_attempts(status);

-- Create attempt_answers table
CREATE TABLE attempt_answers (
    id UUID PRIMARY KEY,
    attempt_id UUID NOT NULL REFERENCES quiz_attempts(id) ON DELETE CASCADE,
    question_id UUID NOT NULL REFERENCES quiz_questions(id) ON DELETE CASCADE,
    chosen_choice_id UUID REFERENCES quiz_choices(id),
    text_answer TEXT,
    is_correct BOOLEAN,
    points_earned INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(attempt_id, question_id)
);

CREATE INDEX idx_attempt_answers_attempt_id ON attempt_answers(attempt_id);

CREATE TRIGGER update_quizzes_updated_at
    BEFORE UPDATE ON quizzes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_quiz_questions_updated_at
    BEFORE UPDATE ON quiz_questions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_quiz_choices_updated_at
    BEFORE UPDATE ON quiz_choices
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_quiz_attempts_updated_at
    BEFORE UPDATE ON quiz_attempts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
