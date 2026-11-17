-- Create ai_feedback_logs table for storing thumbs up / thumbs down student & teacher feedback on AI responses
CREATE TABLE IF NOT EXISTS ai_feedback_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    thread_id VARCHAR(120) NOT NULL,
    message_id VARCHAR(120) NOT NULL,
    rating VARCHAR(20) NOT NULL CHECK (rating IN ('UP', 'DOWN')),
    question_text TEXT,
    ai_reply_text TEXT NOT NULL,
    subject_name VARCHAR(120),
    grade_level VARCHAR(120),
    model_name VARCHAR(120),
    reason TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_ai_feedback_user_msg
    ON ai_feedback_logs(tenant_id, user_id, message_id);

CREATE INDEX IF NOT EXISTS idx_ai_feedback_tenant_rating
    ON ai_feedback_logs(tenant_id, rating, created_at DESC);
