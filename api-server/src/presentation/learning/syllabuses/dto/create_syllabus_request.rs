use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSyllabusRequest {
    pub curriculum_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}
