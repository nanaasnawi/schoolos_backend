-- 20261117230000_add_interaction_config_to_assignment_questions.sql
-- Support interactive game assignment question types: MATCHING, ORDERING, FILL_BLANK
ALTER TABLE assignment_questions
    ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL;
