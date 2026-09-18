use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::assignment_question_dto::SubmitAnswerDto;

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitAssignmentRequest {
    #[serde(default)]
    pub student_id: Option<Uuid>,
    pub content: Option<String>,
    pub file_url: Option<String>,
    #[serde(default)]
    pub answers: Option<Vec<SubmitAnswerDto>>,
}

