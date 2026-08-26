use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::infrastructure::repository_traits::LessonRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteLessonCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub deleted_by: Uuid,
}

pub struct DeleteLessonUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
}

impl DeleteLessonUseCase {
    pub fn new(lesson_repo: Arc<dyn LessonRepository>) -> Self {
        Self { lesson_repo }
    }

    pub async fn execute(&self, command: DeleteLessonCommand) -> Result<(), ApplicationError> {
        let lesson = self
            .lesson_repo
            .find_by_id(command.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonNotFound,
                    format!("Lesson {} not found", command.lesson_id),
                )
            })?;

        self.lesson_repo
            .delete(lesson.id, command.deleted_by)
            .await?;

        Ok(())
    }
}
