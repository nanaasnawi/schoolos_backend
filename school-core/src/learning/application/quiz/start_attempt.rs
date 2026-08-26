use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::quiz_attempt::QuizAttempt;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct StartAttemptCommand {
    pub tenant_id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
}

pub struct StartAttemptUseCase {
    repo: Arc<dyn QuizRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl StartAttemptUseCase {
    pub fn new(
        repo: Arc<dyn QuizRepository>,
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
        command: StartAttemptCommand,
    ) -> Result<QuizAttempt, ApplicationError> {
        let quiz = self
            .repo
            .find_by_id(command.quiz_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::QuizNotFound,
                    format!("Quiz {} not found", command.quiz_id),
                )
            })?;

        let existing_attempts = self.repo.find_attempts_by_quiz(command.quiz_id).await?;
        let previous_count = existing_attempts
            .iter()
            .filter(|a| a.student_id == command.student_id)
            .count() as i32;

        let mut attempt = QuizAttempt::start_new(
            command.tenant_id,
            &quiz,
            command.student_id,
            previous_count,
            &*self.clock,
        )
        .map_err(ApplicationError::Domain)?;

        self.repo.create_attempt(&attempt).await?;

        for event in attempt.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(attempt)
    }
}
