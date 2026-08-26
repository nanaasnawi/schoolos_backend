use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::TeacherCreatedEvent;
use crate::people::domain::teacher::Teacher;
use crate::people::infrastructure::repository_traits::TeacherRepository;

#[derive(Debug, Clone)]
pub struct CreateTeacherCommand {
    pub tenant_id: Uuid,
    pub full_name: String,
    pub nip: Option<String>,
    pub request_id: Option<String>,
}

pub struct CreateTeacherUseCase {
    teacher_repo: Arc<dyn TeacherRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl CreateTeacherUseCase {
    pub fn new(
        teacher_repo: Arc<dyn TeacherRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        uow_factory: Arc<dyn UnitOfWorkFactory>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            teacher_repo,
            outbox_repo,
            uow_factory,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateTeacherCommand,
    ) -> Result<Teacher, ApplicationError> {
        let mut teacher = Teacher::new(
            command.tenant_id,
            command.full_name,
            command.nip,
            &*self.clock,
        );

        let mut uow = self.uow_factory.begin().await?;

        let event = TeacherCreatedEvent {
            metadata: EventMetadata::new(
                "TeacherCreated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            teacher_id: teacher.id,
            nip: teacher.nip.clone(),
            full_name: teacher.full_name.clone(),
        };

        teacher.raise_event(event);
        self.teacher_repo.create(&teacher, &mut *uow).await?;

        for domain_event in teacher.take_events() {
            let outbox_event = OutboxEvent {
                id: Uuid::now_v7(),
                event_type: domain_event.event_name().to_string(),
                payload: domain_event.to_json_value(),
                status: crate::common::domain::outbox::OutboxStatus::Pending,
                created_at: self.clock.now(),
                processed_at: None,
            };
            self.outbox_repo.save(&outbox_event, &mut *uow).await?;
        }

        uow.commit().await?;
        Ok(teacher)
    }
}
