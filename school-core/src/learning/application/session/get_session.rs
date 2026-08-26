use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetSessionQuery {
    pub session_id: Uuid,
}

pub struct GetSessionUseCase {
    session_repo: Arc<dyn SessionRepository>,
}

impl GetSessionUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn execute(
        &self,
        query: GetSessionQuery,
    ) -> Result<LearningSession, ApplicationError> {
        self.session_repo
            .find_by_id(query.session_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::SessionNotFound,
                    format!("Session {} not found", query.session_id),
                )
            })
    }
}
