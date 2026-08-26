use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitAssignmentRequest {
    pub student_id: Uuid,
    pub content: Option<String>,
    pub file_url: Option<String>,
}
