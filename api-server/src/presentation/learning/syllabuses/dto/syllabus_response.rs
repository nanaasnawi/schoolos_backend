use chrono::{DateTime, Utc};
use school_core::learning::domain::syllabus::Syllabus;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SyllabusResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub curriculum_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Syllabus> for SyllabusResponse {
    fn from(s: Syllabus) -> Self {
        Self {
            id: s.id,
            tenant_id: s.tenant_id,
            curriculum_id: s.curriculum_id,
            subject_id: s.subject_id,
            grade_level_id: s.grade_level_id,
            code: s.code,
            name: s.name,
            description: s.description,
            is_active: s.is_active,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}
