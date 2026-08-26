use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::quiz::Quiz;
use crate::learning::infrastructure::repository_traits::{LessonRepository, QuizRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct PublishQuizCommand {
    pub quiz_id: Uuid,
}

pub struct PublishQuizUseCase {
    repo: Arc<dyn QuizRepository>,
    lesson_repo: Arc<dyn LessonRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl PublishQuizUseCase {
    pub fn new(
        repo: Arc<dyn QuizRepository>,
        lesson_repo: Arc<dyn LessonRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            repo,
            lesson_repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(&self, command: PublishQuizCommand) -> Result<Quiz, ApplicationError> {
        let mut quiz = self
            .repo
            .find_by_id(command.quiz_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::QuizNotFound,
                    format!("Quiz {} not found", command.quiz_id),
                )
            })?;

        let lesson = self
            .lesson_repo
            .find_by_id(quiz.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonNotFound,
                    format!("Associated Lesson {} not found", quiz.lesson_id),
                )
            })?;

        quiz.publish(&lesson.status, &*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.repo.update(&quiz).await?;

        for event in quiz.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(quiz)
    }
}
