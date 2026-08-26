use super::command::UpdateStudentCommand;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::error::ApplicationError;
use crate::people::domain::events::StudentUpdatedEvent;
use crate::people::domain::student::Student;
use crate::people::infrastructure::repository_traits::StudentRepository;
use std::sync::Arc;

use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use uuid::Uuid;

pub struct UpdateStudentUseCase {
    student_repo: Arc<dyn StudentRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl UpdateStudentUseCase {
    pub fn new(
        student_repo: Arc<dyn StudentRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        uow_factory: Arc<dyn UnitOfWorkFactory>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            student_repo,
            outbox_repo,
            uow_factory,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: UpdateStudentCommand,
    ) -> Result<Student, ApplicationError> {
        let mut student = self
            .student_repo
            .find_by_id(command.student_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                crate::common::error_code::ErrorCode::StudentNotFound,
                format!("Student not found: {}", command.student_id),
            ))?;

        // Basic verification that the student belongs to the tenant
        if student.tenant_id != command.tenant_id {
            return Err(ApplicationError::Unauthorized(
                crate::common::error_code::ErrorCode::AuthPermissionDenied,
                "Student does not belong to the tenant".to_string(),
            ));
        }

        if let Some(nisn) = command.nisn {
            student.nisn = nisn;
        }

        if let Some(full_name) = command.full_name {
            student.full_name = full_name;
        }

        if let Some(nik) = command.nik {
            student.nik = Some(nik);
        }

        if let Some(gender) = command.gender {
            student.gender = Some(gender);
        }

        if let Some(place_of_birth) = command.place_of_birth {
            student.place_of_birth = Some(place_of_birth);
        }

        if let Some(date_of_birth_str) = command.date_of_birth {
            student.date_of_birth = chrono::NaiveDate::parse_from_str(&date_of_birth_str, "%Y-%m-%d").ok();
        }

        if let Some(religion) = command.religion {
            student.religion = Some(religion);
        }

        student.updated_at = self.clock.now();

        let mut uow = self.uow_factory.begin().await?;

        self.student_repo.update(&student, &mut *uow).await?;

        // Dispatch Event via Outbox
        let event = StudentUpdatedEvent {
            metadata: EventMetadata::new(
                "StudentUpdated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None, // causation_id
                None, // actor_id
                1,    // version
                &*self.clock,
            ),
            student_id: student.id,
        };

        let outbox_event = OutboxEvent {
            id: Uuid::new_v4(),
            event_type: event.metadata.event_type.clone(),
            payload: serde_json::to_value(&event)
                .map_err(|e| ApplicationError::Internal(e.to_string()))?,
            status: crate::common::domain::outbox::OutboxStatus::Pending,
            created_at: self.clock.now(),
            processed_at: None,
        };

        self.outbox_repo.save(&outbox_event, &mut *uow).await?;

        uow.commit().await?;

        Ok(student)
    }
}
