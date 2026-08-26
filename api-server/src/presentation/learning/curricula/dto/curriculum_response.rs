use chrono::{DateTime, Utc};
use school_core::learning::domain::curriculum::Curriculum;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct CurriculumResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Curriculum> for CurriculumResponse {
    fn from(c: Curriculum) -> Self {
        Self {
            id: c.id,
            tenant_id: c.tenant_id,
            code: c.code,
            name: c.name,
            description: c.description,
            is_active: c.is_active,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
