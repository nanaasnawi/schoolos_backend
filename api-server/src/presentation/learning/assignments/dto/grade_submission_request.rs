use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct GradeAnswerDto {
    pub question_id: Uuid,
    pub points_earned: i32,
    pub teacher_feedback: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GradeSubmissionRequest {
    pub score: i32,
    pub feedback: Option<String>,
    #[serde(default)]
    pub answer_grades: Option<Vec<GradeAnswerDto>>,
}

