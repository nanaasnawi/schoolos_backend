-- Create submission_attempts table for tracking revision & attempt history
CREATE TABLE IF NOT EXISTS submission_attempts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    submission_id UUID NOT NULL REFERENCES assignment_submissions(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    content TEXT,
    file_url TEXT,
    checksum VARCHAR(64),
    submitted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    is_late BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(submission_id, attempt_number)
);

CREATE INDEX idx_submission_attempts_submission_id ON submission_attempts(submission_id);
