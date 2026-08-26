use chrono::{DateTime, Utc};
use school_core::learning::domain::syllabus_competency::SyllabusCompetency;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct CompetencyResponse {
    pub id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub competency_type: String,
    pub description: String,
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SyllabusCompetency> for CompetencyResponse {
    fn from(c: SyllabusCompetency) -> Self {
        Self {
            id: c.id,
            syllabus_id: c.syllabus_id,
            code: c.code,
            competency_type: c.competency_type,
            description: c.description,
            order_index: c.order_index,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
