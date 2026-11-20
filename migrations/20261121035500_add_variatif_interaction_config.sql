ALTER TABLE quiz_questions
    ALTER COLUMN question_type TYPE VARCHAR(40);

ALTER TABLE quiz_questions
    ADD COLUMN IF NOT EXISTS interaction_config JSONB;
