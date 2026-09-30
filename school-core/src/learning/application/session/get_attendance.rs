use crate::common::error::ApplicationError;
use crate::learning::domain::session_attendance::SessionAttendance;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetAttendanceQuery {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
}

pub struct GetAttendanceUseCase {
    session_repo: Arc<dyn SessionRepository>,
}

impl GetAttendanceUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn execute(
        &self,
        query: GetAttendanceQuery,
    ) -> Result<Vec<SessionAttendance>, ApplicationError> {
        Ok(self
            .session_repo
            .find_attendance_by_tenant_and_session(query.tenant_id, query.session_id)
            .await?)
    }
}
