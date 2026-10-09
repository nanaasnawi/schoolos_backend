use chrono::{DateTime, NaiveDate, Utc};
use school_core::learning::domain::learning_session::LearningSession;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub session_type: String,
    pub schedule_id: Option<Uuid>,
    pub lesson_id: Option<Uuid>,
    pub class_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Uuid,
    pub substitute_teacher_id: Option<Uuid>,
    pub session_date: NaiveDate,
    pub session_number: i32,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
    pub cancellation_reason: Option<String>,
    pub subject_name: Option<String>,
    pub teacher_name: Option<String>,
    pub substitute_teacher_name: Option<String>,
    pub class_name: Option<String>,
    pub room: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LearningSession> for SessionResponse {
    fn from(s: LearningSession) -> Self {
        Self {
            id: s.id,
            tenant_id: s.tenant_id,
            session_type: s.session_type,
            schedule_id: s.schedule_id,
            lesson_id: s.lesson_id,
            class_id: s.class_id,
            subject_id: s.subject_id,
            teacher_id: s.teacher_id,
            substitute_teacher_id: s.substitute_teacher_id,
            session_date: s.session_date,
            session_number: s.session_number,
            start_time: s.start_time.map(|t| t.format("%H:%M").to_string()),
            end_time: s.end_time.map(|t| t.format("%H:%M").to_string()),
            scheduled_at: s.scheduled_at,
            started_at: s.started_at,
            ended_at: s.ended_at,
            status: s.status,
            notes: s.notes,
            cancellation_reason: s.cancellation_reason,
            subject_name: None,
            teacher_name: None,
            substitute_teacher_name: None,
            class_name: None,
            room: None,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}
