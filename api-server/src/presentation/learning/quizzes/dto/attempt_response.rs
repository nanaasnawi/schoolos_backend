use chrono::{DateTime, Utc};
use school_core::learning::domain::quiz_attempt::QuizAttempt;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::quiz_question_dto::AttemptAnswerDetailDto;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AttemptResponse {
    pub id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub score: Option<i32>,
    pub total_points: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed: Option<bool>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_nisn: Option<String>,
    #[serde(default)]
    pub answers: Vec<AttemptAnswerDetailDto>,
}

impl From<QuizAttempt> for AttemptResponse {
    fn from(a: QuizAttempt) -> Self {
        let percentage = if a.total_points > 0 {
            a.score.map(|s| (s * 100) / a.total_points)
        } else {
            None
        };
        Self {
            id: a.id,
            quiz_id: a.quiz_id,
            student_id: a.student_id,
            started_at: a.started_at,
            completed_at: a.completed_at,
            score: a.score,
            total_points: a.total_points,
            percentage,
            passed: Some(a.passed),
            status: a.status,
            created_at: a.created_at,
            updated_at: a.updated_at,
            student_name: None,
            student_nisn: None,
            answers: Vec::new(),
        }
    }
}
