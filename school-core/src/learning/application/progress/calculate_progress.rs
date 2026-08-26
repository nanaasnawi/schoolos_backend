use crate::common::domain::aggregate::AggregateRoot;
use crate::common::error::ApplicationError;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::student_progress::StudentProgress;
use crate::learning::infrastructure::repository_traits::{
    LessonRepository, SessionRepository, StudentProgressRepository,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct CalculateProgressCommand {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
}

pub struct CalculateProgressUseCase {
    progress_repo: Arc<dyn StudentProgressRepository>,
    lesson_repo: Arc<dyn LessonRepository>,
    session_repo: Arc<dyn SessionRepository>,
    event_bus: SharedEventBus,
}

impl CalculateProgressUseCase {
    pub fn new(
        progress_repo: Arc<dyn StudentProgressRepository>,
        lesson_repo: Arc<dyn LessonRepository>,
        session_repo: Arc<dyn SessionRepository>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            progress_repo,
            lesson_repo,
            session_repo,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: CalculateProgressCommand,
    ) -> Result<StudentProgress, ApplicationError> {
        // Lesson progress — count total lessons for tenant
        let all_lessons = self.lesson_repo.find_by_tenant(command.tenant_id).await?;
        let lesson_total = all_lessons.len() as i32;
        let lesson_completed = all_lessons
            .iter()
            .filter(|l| l.status == "completed" || l.status == "active")
            .count() as i32;

        // Session attendance — count sessions for this class and student's attendance
        let all_sessions = self.session_repo.find_by_class(command.class_id).await?;
        let session_total = all_sessions
            .iter()
            .filter(|s| s.status == "completed")
            .count() as i32;
        let attendance = self
            .session_repo
            .find_attendance_by_student(command.student_id, command.class_id)
            .await?;
        let session_attended = attendance
            .iter()
            .filter(|a| a.status == "present" || a.status == "late")
            .count() as i32;

        let progress = self
            .progress_repo
            .find_by_student_class_subject(command.student_id, command.class_id, Uuid::nil())
            .await?;

        let mut progress = progress.unwrap_or_else(|| {
            StudentProgress::new(
                command.tenant_id,
                command.student_id,
                command.class_id,
                Uuid::nil(),
            )
        });

        progress.update(
            lesson_completed,
            lesson_total,
            0,
            0,
            0,
            0,
            session_attended,
            session_total,
        );

        self.progress_repo.save(&progress).await?;

        progress.emit_updated();
        for event in progress.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(progress)
    }
}
