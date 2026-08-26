use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAttemptRequest {
    pub student_id: Uuid,
}
