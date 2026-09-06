use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAssignmentRequest {
    #[serde(default = "default_lesson_id")]
    pub lesson_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
    #[serde(default = "default_assignment_type")]
    pub assignment_type: String,
    pub class_id: Option<String>,
}

fn default_lesson_id() -> Option<Uuid> {
    Some(Uuid::new_v4())
}

fn default_assignment_type() -> String {
    "individual".to_string()
}
