use chrono::{DateTime, Utc};
use school_core::learning::domain::lesson::Lesson;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: i32,
    pub order_index: i32,
    pub status: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Lesson> for LessonResponse {
    fn from(l: Lesson) -> Self {
        Self {
            id: l.id,
            tenant_id: l.tenant_id,
            syllabus_id: l.syllabus_id,
            code: l.code,
            title: l.title,
            description: l.description,
            learning_objectives: l.learning_objectives,
            duration_minutes: l.duration_minutes,
            order_index: l.order_index,
            status: l.status,
            is_active: l.is_active,
            created_at: l.created_at,
            updated_at: l.updated_at,
        }
    }
}
