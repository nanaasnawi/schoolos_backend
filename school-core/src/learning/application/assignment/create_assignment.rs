use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::assignment::Assignment;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateAssignmentCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: i32,
    pub due_at: Option<DateTime<Utc>>,
    pub assignment_type: String,
}

pub struct CreateAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateAssignmentUseCase {
    pub fn new(repo: Arc<dyn AssignmentRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn execute(
        &self,
        command: CreateAssignmentCommand,
    ) -> Result<Assignment, ApplicationError> {
        let mut assignment = Assignment::new(
            command.tenant_id,
            command.lesson_id,
            command.title,
            command.description,
            command.instructions,
            command.max_score,
            command.due_at,
            command.assignment_type,
            &*self.clock,
        )
        .map_err(ApplicationError::Domain)?;

        self.repo.create(&assignment).await?;

        let _events = assignment.take_events();

        Ok(assignment)
    }
}
