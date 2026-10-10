use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateQuizRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "time_limit_minutes")]
    pub duration_minutes: Option<i32>,
    pub passing_score: Option<i32>,
    pub max_attempts: Option<i32>,
    pub shuffle_questions: Option<bool>,
    pub shuffle_choices: Option<bool>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub class_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub exam_mode: Option<String>,
    pub exam_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub max_token_attempts: Option<i32>,
    pub status: Option<String>,
}
