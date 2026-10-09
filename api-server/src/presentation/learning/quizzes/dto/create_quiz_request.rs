use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateQuizRequest {
    #[serde(default = "default_lesson_id")]
    pub lesson_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    #[serde(alias = "time_limit_minutes")]
    pub duration_minutes: Option<i32>,
    #[serde(default = "default_passing_score")]
    pub passing_score: i32,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: i32,
    #[serde(default)]
    pub shuffle_questions: bool,
    #[serde(default)]
    pub shuffle_choices: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub class_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<Uuid>,
    #[serde(default = "default_exam_mode")]
    pub exam_mode: Option<String>,
    #[serde(default)]
    pub exam_token: Option<String>,
    #[serde(default)]
    pub token_expires_at: Option<DateTime<Utc>>,
    #[serde(default = "default_max_token_attempts")]
    pub max_token_attempts: Option<i32>,
    #[serde(default)]
    pub questions: Option<Vec<super::quiz_question_dto::CreateQuizQuestionRequest>>,
}

fn default_lesson_id() -> Option<Uuid> {
    Some(Uuid::new_v4())
}

fn default_passing_score() -> i32 {
    70
}

fn default_max_attempts() -> i32 {
    1
}

fn default_exam_mode() -> Option<String> {
    Some("HOMEWORK_QUIZ".to_string())
}

fn default_max_token_attempts() -> Option<i32> {
    Some(5)
}
