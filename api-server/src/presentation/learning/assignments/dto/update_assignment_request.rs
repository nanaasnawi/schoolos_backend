use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use super::assignment_question_dto::AssignmentQuestionDto;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAssignmentRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub questions: Option<Vec<AssignmentQuestionDto>>,
}

