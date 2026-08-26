use chrono::{DateTime, Utc};
use school_core::learning::domain::quiz::Quiz;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct QuizResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub duration_minutes: i32,
    pub passing_score: i32,
    pub max_score: i32,
    pub max_attempts: i32,
    pub shuffle_questions: bool,
    pub shuffle_choices: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub status: String,
    pub questions_count: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Quiz> for QuizResponse {
    fn from(q: Quiz) -> Self {
        Self {
            id: q.id,
            tenant_id: q.tenant_id,
            lesson_id: q.lesson_id,
            title: q.title,
            description: q.description,
            duration_minutes: q.duration_minutes,
            passing_score: q.passing_score,
            max_score: q.max_score,
            max_attempts: q.max_attempts,
            shuffle_questions: q.shuffle_questions,
            shuffle_choices: q.shuffle_choices,
            start_at: q.start_at,
            end_at: q.end_at,
            status: q.status,
            questions_count: q.questions_count,
            is_active: q.is_active,
            created_at: q.created_at,
            updated_at: q.updated_at,
        }
    }
}
