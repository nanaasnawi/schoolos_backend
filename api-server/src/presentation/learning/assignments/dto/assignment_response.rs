use chrono::{DateTime, Utc};
use school_core::learning::domain::assignment::Assignment;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AssignmentResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: i32,
    pub due_at: Option<DateTime<Utc>>,
    pub assignment_type: String,
    pub status: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Assignment> for AssignmentResponse {
    fn from(a: Assignment) -> Self {
        Self {
            id: a.id,
            tenant_id: a.tenant_id,
            lesson_id: a.lesson_id,
            title: a.title,
            description: a.description,
            instructions: a.instructions,
            max_score: a.max_score,
            due_at: a.due_at,
            assignment_type: a.assignment_type,
            status: a.status,
            is_active: a.is_active,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}
