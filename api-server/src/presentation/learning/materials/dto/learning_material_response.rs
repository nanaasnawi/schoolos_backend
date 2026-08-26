use chrono::{DateTime, Utc};
use school_core::learning::domain::learning_material::LearningMaterial;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct LearningMaterialResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub material_type: String,
    pub title: String,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub order_index: i32,
    pub visibility: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LearningMaterial> for LearningMaterialResponse {
    fn from(m: LearningMaterial) -> Self {
        Self {
            id: m.id,
            tenant_id: m.tenant_id,
            lesson_id: m.lesson_id,
            material_type: m.material_type,
            title: m.title,
            description: m.description,
            storage_key: m.storage_key,
            external_url: m.external_url,
            order_index: m.order_index,
            visibility: m.visibility,
            is_active: m.is_active,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
