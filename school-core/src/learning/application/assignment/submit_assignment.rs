use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::assignment::AssignmentSubmitted;
use crate::learning::domain::assignment_submission::AssignmentSubmission;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct SubmitAssignmentCommand {
    pub tenant_id: Uuid,
    pub assignment_id: Uuid,
    pub student_id: Uuid,
    pub content: Option<String>,
    pub file_url: Option<String>,
}

pub struct SubmitAssignmentUseCase {
    repo: Arc<dyn AssignmentRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl SubmitAssignmentUseCase {
    pub fn new(
        repo: Arc<dyn AssignmentRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: SubmitAssignmentCommand,
    ) -> Result<AssignmentSubmission, ApplicationError> {
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

        let existing_submissions = self.repo.find_submissions(command.assignment_id).await?;
        let existing = existing_submissions
            .into_iter()
            .find(|s| s.student_id == command.student_id);

        let submission = match existing {
            Some(mut sub) => {
                let attempt = sub
                    .add_attempt(
                        command.content,
                        command.file_url,
                        None,
                        &assignment.status,
                        assignment.due_at,
                        3, // Max 3 attempts
                        &*self.clock,
                    )
                    .map_err(ApplicationError::Domain)?;

                self.repo.update_submission(&sub).await?;
                self.repo.add_attempt(&attempt).await?;
                sub
            }
            None => {
                let sub = AssignmentSubmission::new(
                    command.tenant_id,
                    command.assignment_id,
                    command.student_id,
                    command.content,
                    command.file_url,
                    &*self.clock,
                );

                self.repo.submit(&sub).await?;
                if let Some(first_attempt) = sub.attempts.first() {
                    self.repo.add_attempt(first_attempt).await?;
                }
                sub
            }
        };

        let event: Box<dyn DomainEvent> = Box::new(AssignmentSubmitted {
            metadata: crate::common::domain::event::EventMetadata::new(
                "AssignmentSubmitted".to_string(),
                command.tenant_id,
                assignment.id.to_string(),
                None,
                None,
                None,
                1,
                &*self.clock,
            ),
            submission_id: submission.id,
            assignment_id: command.assignment_id,
            student_id: command.student_id,
            tenant_id: command.tenant_id,
        });
        let _ = self.event_bus.publish(Arc::from(event)).await;

        Ok(submission)
    }
}
