use crate::common::error::ApplicationError;
use crate::learning::domain::session_attendance::SessionAttendance;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceItemDto {
    pub student_id: Uuid,
    pub status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

pub struct RecordAttendanceBulkCommand {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub items: Vec<AttendanceItemDto>,
    pub method: Option<String>,
    pub recorded_by: Option<Uuid>,
}

pub struct RecordAttendanceBulkUseCase {
    session_repo: Arc<dyn SessionRepository>,
}

impl RecordAttendanceBulkUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn execute(
        &self,
        command: RecordAttendanceBulkCommand,
    ) -> Result<Vec<SessionAttendance>, ApplicationError> {
        let attendances: Vec<SessionAttendance> = command
            .items
            .into_iter()
            .map(|item| {
                SessionAttendance::with_details(
                    command.tenant_id,
                    command.session_id,
                    item.student_id,
                    item.status,
                    item.checked_in_at,
                    item.notes,
                    command.method.clone(),
                    command.recorded_by,
                )
            })
            .collect();

        self.session_repo
            .record_attendance_bulk(&attendances)
            .await?;

        Ok(attendances)
    }
}
