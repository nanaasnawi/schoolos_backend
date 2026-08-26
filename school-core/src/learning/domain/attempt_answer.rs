use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptAnswer {
    pub id: Uuid,
    pub attempt_id: Uuid,
    pub question_id: Uuid,
    pub chosen_choice_id: Option<Uuid>,
    pub text_answer: Option<String>,
    pub is_correct: Option<bool>,
    pub points_earned: i32,
    pub created_at: DateTime<Utc>,
}

impl AttemptAnswer {
    pub fn new(
        attempt_id: Uuid,
        question_id: Uuid,
        chosen_choice_id: Option<Uuid>,
        text_answer: Option<String>,
    ) -> Self {
        assert!(!attempt_id.is_nil(), "attempt_id must not be nil");
        assert!(!question_id.is_nil(), "question_id must not be nil");

        Self {
            id: Uuid::now_v7(),
            attempt_id,
            question_id,
            chosen_choice_id,
            text_answer,
            is_correct: None,
            points_earned: 0,
            created_at: Utc::now(),
        }
    }

    pub fn rehydrate(
        id: Uuid,
        attempt_id: Uuid,
        question_id: Uuid,
        chosen_choice_id: Option<Uuid>,
        text_answer: Option<String>,
        is_correct: Option<bool>,
        points_earned: i32,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            attempt_id,
            question_id,
            chosen_choice_id,
            text_answer,
            is_correct,
            points_earned,
            created_at,
        }
    }
}
