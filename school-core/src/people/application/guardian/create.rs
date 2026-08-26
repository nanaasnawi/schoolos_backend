use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::GuardianCreatedEvent;
use crate::people::domain::guardian::Guardian;
use crate::people::infrastructure::repository_traits::GuardianRepository;

#[derive(Debug, Clone)]
pub struct CreateGuardianCommand {
    pub tenant_id: Uuid,
    pub full_name: String,
    pub phone_number: Option<String>,
    pub request_id: Option<String>,
}

pub struct CreateGuardianUseCase {
    guardian_repo: Arc<dyn GuardianRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl CreateGuardianUseCase {
    pub fn new(
        guardian_repo: Arc<dyn GuardianRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        uow_factory: Arc<dyn UnitOfWorkFactory>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            guardian_repo,
            outbox_repo,
            uow_factory,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateGuardianCommand,
    ) -> Result<Guardian, ApplicationError> {
        let mut guardian = Guardian::new(
            command.tenant_id,
            command.full_name,
            command.phone_number,
            &*self.clock,
        );

        let mut uow = self.uow_factory.begin().await?;

        let event = GuardianCreatedEvent {
            metadata: EventMetadata::new(
                "GuardianCreated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            guardian_id: guardian.id,
            full_name: guardian.full_name.clone(),
        };

        guardian.raise_event(event);
        self.guardian_repo.create(&guardian, &mut *uow).await?;

        for domain_event in guardian.take_events() {
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
        Ok(guardian)
    }
}
