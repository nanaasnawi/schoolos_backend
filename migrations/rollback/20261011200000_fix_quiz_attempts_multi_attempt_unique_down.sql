-- Rollback: Revert quiz_attempts unique constraint back to (quiz_id, student_id)
ALTER TABLE quiz_attempts DROP CONSTRAINT IF EXISTS uq_quiz_attempts_quiz_student_attempt;
ALTER TABLE quiz_attempts ADD CONSTRAINT quiz_attempts_quiz_id_student_id_key UNIQUE (quiz_id, student_id);
