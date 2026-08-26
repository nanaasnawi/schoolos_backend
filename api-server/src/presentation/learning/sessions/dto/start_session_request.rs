use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartSessionRequest {
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub notes: Option<String>,
}
