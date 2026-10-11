-- Fix quiz_attempts unique constraint to support max_attempts > 1
-- Previously, UNIQUE(quiz_id, student_id) prevented students from retrying quizzes even when max_attempts > 1.
ALTER TABLE quiz_attempts DROP CONSTRAINT IF EXISTS quiz_attempts_quiz_id_student_id_key;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'uq_quiz_attempts_quiz_student_attempt'
    ) THEN
        ALTER TABLE quiz_attempts ADD CONSTRAINT uq_quiz_attempts_quiz_student_attempt UNIQUE (quiz_id, student_id, attempt_number);
    END IF;
END $$;
