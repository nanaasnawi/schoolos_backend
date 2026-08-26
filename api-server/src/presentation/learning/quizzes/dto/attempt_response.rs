use chrono::{DateTime, Utc};
use school_core::learning::domain::quiz_attempt::QuizAttempt;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AttemptResponse {
    pub id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub score: Option<i32>,
    pub total_points: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<QuizAttempt> for AttemptResponse {
    fn from(a: QuizAttempt) -> Self {
        Self {
            id: a.id,
            quiz_id: a.quiz_id,
            student_id: a.student_id,
            started_at: a.started_at,
            completed_at: a.completed_at,
            score: a.score,
            total_points: a.total_points,
            status: a.status,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}
