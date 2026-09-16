-- 20260917030000_create_assignment_questions.sql
-- Unified Assessment Model: Support Multiple Choice (PG) + Essay on Assignments

-- 1. Create assignment_questions table
CREATE TABLE IF NOT EXISTS assignment_questions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    assignment_id UUID NOT NULL REFERENCES assignments(id) ON DELETE CASCADE,
    question_text TEXT NOT NULL,
    question_type VARCHAR(30) NOT NULL DEFAULT 'MULTIPLE_CHOICE', -- 'MULTIPLE_CHOICE' or 'ESSAY'
    points INTEGER NOT NULL DEFAULT 10,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_assignment_questions_assignment ON assignment_questions(assignment_id);
CREATE INDEX IF NOT EXISTS idx_assignment_questions_tenant ON assignment_questions(tenant_id);

-- 2. Create assignment_question_choices table for PG options
CREATE TABLE IF NOT EXISTS assignment_question_choices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    question_id UUID NOT NULL REFERENCES assignment_questions(id) ON DELETE CASCADE,
    choice_text TEXT NOT NULL,
    is_correct BOOLEAN NOT NULL DEFAULT false,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_assignment_choices_question ON assignment_question_choices(question_id);

-- 3. Create assignment_submission_answers table for student answers & grading
CREATE TABLE IF NOT EXISTS assignment_submission_answers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    submission_id UUID NOT NULL REFERENCES assignment_submissions(id) ON DELETE CASCADE,
    question_id UUID NOT NULL REFERENCES assignment_questions(id) ON DELETE CASCADE,
    chosen_choice_id UUID REFERENCES assignment_question_choices(id) ON DELETE SET NULL,
    text_answer TEXT,
    points_earned INTEGER DEFAULT 0,
    teacher_feedback TEXT,
    graded_at TIMESTAMP WITH TIME ZONE,
    graded_by UUID,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(submission_id, question_id)
);

CREATE INDEX IF NOT EXISTS idx_assignment_answers_submission ON assignment_submission_answers(submission_id);
