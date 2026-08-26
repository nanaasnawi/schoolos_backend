use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::assignment::Assignment;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateAssignmentCommand {
    pub tenant_id: Uuid,
    pub assignment_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
}

pub struct UpdateAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
    clock: Arc<dyn Clock>,
}

impl UpdateAssignmentUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn execute(
        &self,
        command: UpdateAssignmentCommand,
    ) -> Result<Assignment, ApplicationError> {
        let mut assignment = self
            .repo
            .find_by_id(command.assignment_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssignmentNotFound,
                    format!("Assignment {} not found", command.assignment_id),
                )
            })?;

        assignment
            .update(
                command.title,
                command.description,
                command.instructions,
                command.max_score,
                command.due_at,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?;

        self.repo.update(&assignment).await?;

        let _events = assignment.take_events();

        Ok(assignment)
    }
}
