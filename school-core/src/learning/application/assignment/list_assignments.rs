use crate::common::error::ApplicationError;
use crate::learning::domain::assignment::Assignment;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListAssignmentsQuery {
    pub tenant_id: Uuid,
}

pub struct ListAssignmentsUseCase {
    repo: Arc<dyn AssignmentRepository>,
}

impl ListAssignmentsUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: ListAssignmentsQuery,
    ) -> Result<Vec<Assignment>, ApplicationError> {
        Ok(self.repo.find_by_tenant(query.tenant_id).await?)
    }
}
