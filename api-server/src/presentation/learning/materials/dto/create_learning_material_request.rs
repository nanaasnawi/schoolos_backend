use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLearningMaterialRequest {
    pub lesson_id: Option<Uuid>,
    pub material_type: String,
    pub title: String,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub order_index: i32,
    pub visibility: String,
}
