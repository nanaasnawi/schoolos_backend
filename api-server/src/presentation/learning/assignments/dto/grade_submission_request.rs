use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct GradeSubmissionRequest {
    pub score: i32,
    pub feedback: Option<String>,
}
