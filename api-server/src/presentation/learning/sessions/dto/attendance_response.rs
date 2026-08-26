use chrono::{DateTime, Utc};
use school_core::learning::domain::session_attendance::SessionAttendance;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AttendanceResponse {
    pub id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SessionAttendance> for AttendanceResponse {
    fn from(a: SessionAttendance) -> Self {
        Self {
            id: a.id,
            session_id: a.session_id,
            student_id: a.student_id,
            status: a.status,
            checked_in_at: a.checked_in_at,
            notes: a.notes,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}
