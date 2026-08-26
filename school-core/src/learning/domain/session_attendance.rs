use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionAttendance {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Clone for SessionAttendance {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            session_id: self.session_id,
            student_id: self.student_id,
            status: self.status.clone(),
            checked_in_at: self.checked_in_at,
            notes: self.notes.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl SessionAttendance {
    pub fn new(
        tenant_id: Uuid,
        session_id: Uuid,
        student_id: Uuid,
        status: String,
        checked_in_at: Option<DateTime<Utc>>,
        notes: Option<String>,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!session_id.is_nil(), "session_id must not be nil");
        assert!(!student_id.is_nil(), "student_id must not be nil");

        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            session_id,
            student_id,
            status,
            checked_in_at,
            notes,
            created_at: now,
            updated_at: now,
        }
    }
}
