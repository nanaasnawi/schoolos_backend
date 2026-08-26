-- Add minimum_passing_grade, status, rounding_policy, and gradebooks table for Assessment Bounded Context
ALTER TABLE assessment_rules
    ADD COLUMN IF NOT EXISTS minimum_passing_grade NUMERIC(5,2) NOT NULL DEFAULT 70.0,
    ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'draft',
    ADD COLUMN IF NOT EXISTS rounding_policy VARCHAR(20) NOT NULL DEFAULT 'half_up';

ALTER TABLE assessment_rule_components
    ADD COLUMN IF NOT EXISTS is_required BOOLEAN NOT NULL DEFAULT true;

-- Create gradebooks table
CREATE TABLE IF NOT EXISTS gradebooks (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    academic_year_id UUID,
    final_score NUMERIC(5,2),
    letter_grade VARCHAR(5),
    passed BOOLEAN,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, class_id, subject_id)
);

CREATE INDEX idx_gradebooks_student_id ON gradebooks(student_id);
CREATE INDEX idx_gradebooks_class_subject ON gradebooks(class_id, subject_id);
CREATE INDEX idx_gradebooks_status ON gradebooks(status);

ALTER TABLE gradebook_entries
    ADD COLUMN IF NOT EXISTS gradebook_id UUID REFERENCES gradebooks(id) ON DELETE CASCADE;
