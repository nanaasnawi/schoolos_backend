use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::lesson::Lesson;
use crate::learning::infrastructure::repository_traits::LessonRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateLessonCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: Option<i32>,
}

pub struct UpdateLessonUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
    clock: Arc<dyn Clock>,
}

impl UpdateLessonUseCase {
    pub fn new(lesson_repo: Arc<dyn LessonRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { lesson_repo, clock }
    }

    pub async fn execute(&self, command: UpdateLessonCommand) -> Result<Lesson, ApplicationError> {
        let mut lesson = self
            .lesson_repo
            .find_by_id(command.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonNotFound,
                    format!("Lesson {} not found", command.lesson_id),
                )
            })?;

        lesson
            .update(
                command.title,
                command.description,
                command.learning_objectives,
                command.duration_minutes,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?;

        self.lesson_repo.update(&lesson).await?;

        let _events = lesson.take_events();

        Ok(lesson)
    }
}
