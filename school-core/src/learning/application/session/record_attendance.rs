use crate::common::error::ApplicationError;
use crate::learning::domain::session_attendance::SessionAttendance;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct RecordAttendanceCommand {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

pub struct RecordAttendanceUseCase {
    session_repo: Arc<dyn SessionRepository>,
}

impl RecordAttendanceUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn execute(
        &self,
        command: RecordAttendanceCommand,
    ) -> Result<SessionAttendance, ApplicationError> {
        let attendance = SessionAttendance::new(
            command.tenant_id,
            command.session_id,
            command.student_id,
            command.status,
            command.checked_in_at,
            command.notes,
        );

        self.session_repo.record_attendance(&attendance).await?;

        Ok(attendance)
    }
}
