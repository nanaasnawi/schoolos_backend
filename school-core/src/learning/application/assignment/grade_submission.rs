use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::assignment::GradeReleased;
use crate::learning::domain::assignment_submission::AssignmentSubmission;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GradeSubmissionCommand {
    pub tenant_id: Uuid,
    pub submission_id: Uuid,
    pub score: i32,
    pub feedback: Option<String>,
    pub graded_by: Uuid,
}

pub struct GradeSubmissionUseCase {
    repo: Arc<dyn AssignmentRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl GradeSubmissionUseCase {
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
        command: GradeSubmissionCommand,
    ) -> Result<AssignmentSubmission, ApplicationError> {
        let mut submission = self
            .repo
            .find_submission_by_id(command.submission_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::SubmissionNotFound,
                    format!("Submission {} not found", command.submission_id),
                )
            })?;

        let assignment = self
            .repo
            .find_by_id(submission.assignment_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssignmentNotFound,
                    format!("Assignment {} not found", submission.assignment_id),
                )
            })?;

        submission
            .grade(
                command.score,
                assignment.max_score,
                command.feedback,
                command.graded_by,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?;

        self.repo.update_submission(&submission).await?;

        let event: Box<dyn DomainEvent> = Box::new(GradeReleased {
            metadata: crate::common::domain::event::EventMetadata::new(
                "GradeReleased".to_string(),
                submission.tenant_id,
                submission.assignment_id.to_string(),
                None,
                None,
                None,
                1,
                &*self.clock,
            ),
            submission_id: submission.id,
            assignment_id: submission.assignment_id,
            student_id: submission.student_id,
            score: command.score,
            tenant_id: submission.tenant_id,
        });
        let _ = self.event_bus.publish(Arc::from(event)).await;

        Ok(submission)
    }
}
