use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateQuizRequest {
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
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
}

fn default_passing_score() -> i32 {
    70
}

fn default_max_attempts() -> i32 {
    1
}
