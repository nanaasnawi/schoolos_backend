use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::lesson::Lesson;
use crate::learning::infrastructure::repository_traits::{
    LearningMaterialRepository, LessonRepository,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct PublishLessonCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
}

pub struct PublishLessonUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
    material_repo: Arc<dyn LearningMaterialRepository>,
    clock: Arc<dyn Clock>,
}

impl PublishLessonUseCase {
    pub fn new(
        lesson_repo: Arc<dyn LessonRepository>,
        material_repo: Arc<dyn LearningMaterialRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            lesson_repo,
            material_repo,
            clock,
        }
    }

    pub async fn execute(&self, command: PublishLessonCommand) -> Result<Lesson, ApplicationError> {
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

        let materials = self.material_repo.find_by_tenant(command.tenant_id).await?;
        let lesson_materials_count = materials
            .into_iter()
            .filter(|m| m.lesson_id == Some(lesson.id))
            .count();

        lesson
            .publish(lesson_materials_count, &*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.lesson_repo.update(&lesson).await?;

        let _events = lesson.take_events();

        Ok(lesson)
    }
}
