use chrono::{DateTime, Utc};
use school_core::learning::domain::learning_session::LearningSession;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LearningSession> for SessionResponse {
    fn from(s: LearningSession) -> Self {
        Self {
            id: s.id,
            tenant_id: s.tenant_id,
            lesson_id: s.lesson_id,
            class_id: s.class_id,
            teacher_id: s.teacher_id,
            scheduled_at: s.scheduled_at,
            started_at: s.started_at,
            ended_at: s.ended_at,
            status: s.status,
            notes: s.notes,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}
