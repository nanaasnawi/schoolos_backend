use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizChoice {
    pub id: Uuid,
    pub question_id: Uuid,
    pub choice_text: String,
    pub is_correct: bool,
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl QuizChoice {
    pub fn new(question_id: Uuid, choice_text: String, is_correct: bool, order_index: i32) -> Self {
        assert!(!question_id.is_nil(), "question_id must not be nil");
        assert!(!choice_text.is_empty(), "choice_text must not be empty");

        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            question_id,
            choice_text,
            is_correct,
            order_index,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn rehydrate(
        id: Uuid,
        question_id: Uuid,
        choice_text: String,
        is_correct: bool,
        order_index: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            question_id,
            choice_text,
            is_correct,
            order_index,
            created_at,
            updated_at,
        }
    }
}
