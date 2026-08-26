use crate::common::error::ApplicationError;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListSessionsQuery {
    pub tenant_id: Uuid,
}

pub struct ListSessionsUseCase {
    session_repo: Arc<dyn SessionRepository>,
}

impl ListSessionsUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn execute(
        &self,
        query: ListSessionsQuery,
    ) -> Result<Vec<LearningSession>, ApplicationError> {
        Ok(self.session_repo.find_by_tenant(query.tenant_id).await?)
    }
}
