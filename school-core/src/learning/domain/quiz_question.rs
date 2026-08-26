use crate::learning::domain::quiz_choice::QuizChoice;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestion {
    pub id: Uuid,
    pub quiz_id: Uuid,
    pub question_text: String,
    pub question_type: String,
    pub points: i32,
    pub order_index: i32,
    pub choices: Vec<QuizChoice>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl QuizQuestion {
    pub fn new(
        quiz_id: Uuid,
        question_text: String,
        question_type: String,
        points: i32,
        order_index: i32,
        choices: Vec<QuizChoice>,
    ) -> Self {
        assert!(!quiz_id.is_nil(), "quiz_id must not be nil");
        assert!(!question_text.is_empty(), "question_text must not be empty");

        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            quiz_id,
            question_text,
            question_type,
            points,
            order_index,
            choices,
            created_at: now,
            updated_at: now,
        }
    }

    /// Calculate total points for this question based on choices marked correct.
    pub fn max_points(&self) -> i32 {
        if self.question_type == "multiple_choice" || self.question_type == "true_false" {
            if self.choices.iter().any(|c| c.is_correct) {
                self.points
            } else {
                0
            }
        } else {
            self.points
        }
    }

    pub fn rehydrate(
        id: Uuid,
        quiz_id: Uuid,
        question_text: String,
        question_type: String,
        points: i32,
        order_index: i32,
        choices: Vec<QuizChoice>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            quiz_id,
            question_text,
            question_type,
            points,
            order_index,
            choices,
            created_at,
            updated_at,
        }
    }
}
