use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AssignmentChoiceDto {
    pub id: Option<Uuid>,
    pub choice_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_correct: Option<bool>,
    pub order_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AssignmentQuestionDto {
    pub id: Option<Uuid>,
    pub question_text: String,
    #[serde(default = "default_question_type")]
    pub question_type: String, // "MULTIPLE_CHOICE" or "ESSAY"
    pub points: Option<i32>,
    pub order_index: Option<i32>,
    #[serde(default)]
    pub choices: Vec<AssignmentChoiceDto>,
}

fn default_question_type() -> String {
    "MULTIPLE_CHOICE".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitAnswerDto {
    pub question_id: Uuid,
    pub chosen_choice_id: Option<Uuid>,
    pub text_answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmissionAnswerDetailDto {
    pub question_id: Uuid,
    pub question_text: String,
    pub question_type: String,
    pub max_points: i32,
    pub chosen_choice_id: Option<Uuid>,
    pub chosen_choice_text: Option<String>,
    pub is_correct: Option<bool>,
    pub text_answer: Option<String>,
    pub points_earned: i32,
    pub teacher_feedback: Option<String>,
}
