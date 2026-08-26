use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteAssignmentCommand {
    pub tenant_id: Uuid,
    pub assignment_id: Uuid,
    pub deleted_by: Uuid,
}

pub struct DeleteAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
}

impl DeleteAssignmentUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, command: DeleteAssignmentCommand) -> Result<(), ApplicationError> {
        let assignment = self
            .repo
            .find_by_id(command.assignment_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssignmentNotFound,
                    format!("Assignment {} not found", command.assignment_id),
                )
            })?;

        self.repo.delete(assignment.id, command.deleted_by).await?;

        Ok(())
    }
}
