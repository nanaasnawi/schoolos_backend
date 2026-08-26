use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::lesson::Lesson;
use crate::learning::infrastructure::repository_traits::LessonRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetLessonQuery {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
}

pub struct GetLessonUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
}

impl GetLessonUseCase {
    pub fn new(lesson_repo: Arc<dyn LessonRepository>) -> Self {
        Self { lesson_repo }
    }

    pub async fn execute(&self, query: GetLessonQuery) -> Result<Lesson, ApplicationError> {
        self.lesson_repo
            .find_by_id(query.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonNotFound,
                    format!("Lesson {} not found", query.lesson_id),
                )
            })
    }
}
