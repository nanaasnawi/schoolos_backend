use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CalculateProgressRequest {
    pub student_id: Uuid,
    pub class_id: Uuid,
}
