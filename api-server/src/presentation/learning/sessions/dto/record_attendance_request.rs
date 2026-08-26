use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecordAttendanceRequest {
    pub student_id: Uuid,
    pub status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}
