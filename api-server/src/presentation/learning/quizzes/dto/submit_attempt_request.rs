use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitAnswerRequest {
    pub question_id: Uuid,
    pub chosen_choice_id: Option<Uuid>,
    pub text_answer: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitAttemptRequest {
    pub answers: Vec<SubmitAnswerRequest>,
}
