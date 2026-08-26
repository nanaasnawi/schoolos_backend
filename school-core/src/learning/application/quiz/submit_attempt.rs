use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::attempt_answer::AttemptAnswer;
use crate::learning::domain::quiz_attempt::QuizAttempt;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct SubmitAnswer {
    pub question_id: Uuid,
    pub chosen_choice_id: Option<Uuid>,
    pub text_answer: Option<String>,
}

pub struct SubmitAttemptCommand {
    pub attempt_id: Uuid,
    pub answers: Vec<SubmitAnswer>,
}

pub struct SubmitAttemptUseCase {
    repo: Arc<dyn QuizRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl SubmitAttemptUseCase {
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
        command: SubmitAttemptCommand,
    ) -> Result<QuizAttempt, ApplicationError> {
        let mut attempt = self
            .repo
            .find_attempt_by_id(command.attempt_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AttemptNotFound,
                    format!("Attempt {} not found", command.attempt_id),
                )
            })?;

        let quiz = self
            .repo
            .find_by_id(attempt.quiz_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::QuizNotFound,
                    format!("Quiz {} not found", attempt.quiz_id),
                )
            })?;

        if attempt.status != "in_progress" {
            return Err(ApplicationError::Domain(
                crate::common::error::DomainError::Validation(
                    "Attempt is not in progress".to_string(),
                ),
            ));
        }

        for answer in command.answers {
            let ans = AttemptAnswer::new(
                command.attempt_id,
                answer.question_id,
                answer.chosen_choice_id,
                answer.text_answer,
            );
            self.repo.save_answer(&ans).await?;
            attempt.add_answer(ans);
        }

        attempt
            .auto_grade(&quiz, &*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.repo.update_attempt(&attempt).await?;

        for event in attempt.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(attempt)
    }
}
