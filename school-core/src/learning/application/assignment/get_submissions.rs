use crate::common::error::ApplicationError;
use crate::learning::domain::assignment_submission::AssignmentSubmission;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetSubmissionsQuery {
    pub assignment_id: Uuid,
}

pub struct GetSubmissionsUseCase {
    repo: Arc<dyn AssignmentRepository>,
}

impl GetSubmissionsUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetSubmissionsQuery,
    ) -> Result<Vec<AssignmentSubmission>, ApplicationError> {
        Ok(self.repo.find_submissions(query.assignment_id).await?)
    }
}
