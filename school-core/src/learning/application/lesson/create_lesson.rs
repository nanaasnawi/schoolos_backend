use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::lesson::Lesson;
use crate::learning::infrastructure::repository_traits::LessonRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateLessonCommand {
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: i32,
    pub order_index: i32,
    pub status: String,
}

pub struct CreateLessonUseCase {
    lesson_repo: Arc<dyn LessonRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateLessonUseCase {
    pub fn new(lesson_repo: Arc<dyn LessonRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { lesson_repo, clock }
    }

    pub async fn execute(&self, command: CreateLessonCommand) -> Result<Lesson, ApplicationError> {
        let mut lesson = Lesson::new(
            command.tenant_id,
            command.syllabus_id,
            command.code,
            command.title,
            command.description,
            command.learning_objectives,
            command.duration_minutes,
            command.order_index,
            command.status,
            &*self.clock,
        );

        self.lesson_repo.create(&lesson).await?;

        let _events = lesson.take_events();

        Ok(lesson)
    }
}
