use chrono::{DateTime, Utc};
use school_core::academic::domain::subject::Subject;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SubjectResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Subject> for SubjectResponse {
    fn from(subject: Subject) -> Self {
        Self {
            id: subject.id,
            tenant_id: subject.tenant_id,
            code: subject.code,
            name: subject.name,
            is_active: subject.is_active,
            created_at: subject.created_at,
            updated_at: subject.updated_at,
        }
    }
}
