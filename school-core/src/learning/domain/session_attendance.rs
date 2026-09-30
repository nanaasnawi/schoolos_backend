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
    pub method: Option<String>,
    pub recorded_by: Option<Uuid>,
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
            method: self.method.clone(),
            recorded_by: self.recorded_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl SessionAttendance {
    pub fn normalize_status(raw: &str) -> String {
        match raw.trim().to_lowercase().as_str() {
            "present" | "hadir" | "h" => "present".to_string(),
            "sick" | "sakit" | "s" => "sick".to_string(),
            "excused" | "izin" | "i" => "excused".to_string(),
            "late" | "terlambat" | "t" => "late".to_string(),
            _ => "absent".to_string(),
        }
    }

    pub fn new(
        tenant_id: Uuid,
        session_id: Uuid,
        student_id: Uuid,
        status: String,
        checked_in_at: Option<DateTime<Utc>>,
        notes: Option<String>,
    ) -> Self {
        let now = Utc::now();
        let valid_checked_in = if status.to_lowercase() == "present" || status.to_lowercase() == "late" {
            checked_in_at.or(Some(now))
        } else {
            checked_in_at
        };
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            session_id,
            student_id,
            status: Self::normalize_status(&status),
            checked_in_at: valid_checked_in,
            notes,
            method: Some("manual".to_string()),
            recorded_by: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_details(
        tenant_id: Uuid,
        session_id: Uuid,
        student_id: Uuid,
        status: String,
        checked_in_at: Option<DateTime<Utc>>,
        notes: Option<String>,
        method: Option<String>,
        recorded_by: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        let valid_checked_in = if status.to_lowercase() == "present" || status.to_lowercase() == "late" {
            checked_in_at.or(Some(now))
        } else {
            checked_in_at
        };
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            session_id,
            student_id,
            status: Self::normalize_status(&status),
            checked_in_at: valid_checked_in,
            notes,
            method: method.or(Some("manual".to_string())),
            recorded_by,
            created_at: now,
            updated_at: now,
        }
    }
}
