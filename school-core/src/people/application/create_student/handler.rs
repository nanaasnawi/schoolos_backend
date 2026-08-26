use super::command::CreateStudentCommand;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::error::ApplicationError;
use crate::people::domain::events::StudentCreatedEvent;
use crate::people::domain::student::Student;
use crate::people::infrastructure::repository_traits::StudentRepository;
use std::sync::Arc;

use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use uuid::Uuid;

pub struct CreateStudentUseCase {
    student_repo: Arc<dyn StudentRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl CreateStudentUseCase {
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
        command: CreateStudentCommand,
    ) -> Result<Student, ApplicationError> {
        let date_of_birth = if let Some(dob_str) = command.date_of_birth.as_ref() {
            chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d").ok()
        } else {
            None
        };

        let mut student = Student::register(
            command.tenant_id,
            command.nisn.clone(),
            command.full_name,
            command.nik,
            command.gender,
            command.place_of_birth,
            date_of_birth,
            command.religion,
            None, // nipd
            None, // alamat_jalan
            None, // no_hp
            None, // email
            command.guardian_id,
            &*self.clock,
        )
        .map_err(|e| ApplicationError::Domain(crate::common::error::DomainError::Validation(e)))?;

        let mut uow = self.uow_factory.begin().await?;

        // Dispatch Event via Aggregate
        let event = StudentCreatedEvent {
            metadata: EventMetadata::new(
                "StudentCreated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None, // causation_id
                None, // actor_id
                1,    // version
                &*self.clock,
            ),
            student_id: student.id,
            nisn: Some(student.nisn.clone()),
            nipd: None,
        };

        student.raise_event(event);

        self.student_repo.create(&student, &mut *uow).await?;

        // Collect events to Outbox
        for domain_event in student.take_events() {
            let outbox_event = OutboxEvent {
                id: Uuid::new_v4(),
                event_type: domain_event.event_name().to_string(),
                payload: domain_event.to_json_value(),
                status: crate::common::domain::outbox::OutboxStatus::Pending,
                created_at: self.clock.now(),
                processed_at: None,
            };

            self.outbox_repo.save(&outbox_event, &mut *uow).await?;
        }

        uow.commit().await?;

        Ok(student)
    }
}
