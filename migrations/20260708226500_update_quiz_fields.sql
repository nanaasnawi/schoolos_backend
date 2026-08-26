-- Add additional fields to quizzes and quiz_attempts tables for rich quiz & attempt invariants
ALTER TABLE quizzes 
    ADD COLUMN IF NOT EXISTS max_attempts INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS shuffle_questions BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS shuffle_choices BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS start_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS end_at TIMESTAMP WITH TIME ZONE;

ALTER TABLE quiz_attempts
    ADD COLUMN IF NOT EXISTS attempt_number INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS passed BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS shuffle_seed BIGINT NOT NULL DEFAULT 0;
