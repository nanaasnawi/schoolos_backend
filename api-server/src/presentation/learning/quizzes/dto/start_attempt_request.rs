use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAttemptRequest {
    #[serde(default)]
    pub student_id: Option<Uuid>,
    #[serde(default, alias = "token", alias = "examToken")]
    pub exam_token: Option<String>,
}
