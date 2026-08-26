use crate::common::error::ApplicationError;
use crate::learning::domain::lesson::Lesson;
use crate::learning::infrastructure::repository_traits::LessonRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListLessonsQuery {
    pub tenant_id: Uuid,
}

pub struct ListLessonsUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
}

impl ListLessonsUseCase {
    pub fn new(lesson_repo: Arc<dyn LessonRepository>) -> Self {
        Self { lesson_repo }
    }

    pub async fn execute(&self, query: ListLessonsQuery) -> Result<Vec<Lesson>, ApplicationError> {
        Ok(self.lesson_repo.find_by_tenant(query.tenant_id).await?)
    }
}
