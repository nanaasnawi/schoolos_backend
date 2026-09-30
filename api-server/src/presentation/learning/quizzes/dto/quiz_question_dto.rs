use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateQuizOptionInput {
    pub choice_text: String,
    pub is_correct: Option<bool>,
    pub order_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateQuizQuestionRequest {
    pub question_text: String,
    pub question_type: Option<String>,
    pub points: Option<i32>,
    pub order_index: Option<i32>,
    pub image_url: Option<String>,
    pub choices: Option<Vec<CreateQuizOptionInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuizChoiceResponse {
    pub id: Uuid,
    pub choice_text: String,
    pub order_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_correct: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuizQuestionResponse {
    pub id: Uuid,
    pub question_text: String,
    pub question_type: String,
    pub points: i32,
    pub order_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub choices: Vec<QuizChoiceResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AttemptAnswerDetailDto {
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

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct GradeAttemptAnswerDto {
    pub question_id: Uuid,
    pub points_earned: i32,
    pub teacher_feedback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct GradeAttemptRequest {
    pub score: Option<i32>,
    pub feedback: Option<String>,
    pub answer_grades: Option<Vec<GradeAttemptAnswerDto>>,
}
