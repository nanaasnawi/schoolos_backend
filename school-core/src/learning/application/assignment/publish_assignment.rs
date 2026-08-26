use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::assignment::Assignment;
use crate::learning::infrastructure::repository_traits::{AssignmentRepository, LessonRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct PublishAssignmentCommand {
    pub tenant_id: Uuid,
    pub assignment_id: Uuid,
}

pub struct PublishAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
    lesson_repo: Arc<dyn LessonRepository>,
    clock: Arc<dyn Clock>,
}

impl PublishAssignmentUseCase {
    pub fn new(
        repo: Arc<dyn AssignmentRepository>,
        lesson_repo: Arc<dyn LessonRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repo,
            lesson_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: PublishAssignmentCommand,
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

        let lesson = self
            .lesson_repo
            .find_by_id(assignment.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonNotFound,
                    format!("Associated Lesson {} not found", assignment.lesson_id),
                )
            })?;

        assignment
            .publish(&lesson.status, &*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.repo.update(&assignment).await?;

        let _events = assignment.take_events();

        Ok(assignment)
    }
}
