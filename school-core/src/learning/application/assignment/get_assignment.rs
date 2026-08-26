use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::assignment::Assignment;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetAssignmentQuery {
    pub assignment_id: Uuid,
}

pub struct GetAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
}

impl GetAssignmentUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: GetAssignmentQuery) -> Result<Assignment, ApplicationError> {
        self.repo
            .find_by_id(query.assignment_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssignmentNotFound,
                    format!("Assignment {} not found", query.assignment_id),
                )
            })
    }
}
