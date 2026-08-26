use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::StaffCreatedEvent;
use crate::people::domain::staff::Staff;
use crate::people::infrastructure::repository_traits::StaffRepository;

#[derive(Debug, Clone)]
pub struct CreateStaffCommand {
    pub tenant_id: Uuid,
    pub full_name: String,
    pub job_title: String,
    pub request_id: Option<String>,
}

pub struct CreateStaffUseCase {
    staff_repo: Arc<dyn StaffRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl CreateStaffUseCase {
    pub fn new(
        staff_repo: Arc<dyn StaffRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        uow_factory: Arc<dyn UnitOfWorkFactory>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            staff_repo,
            outbox_repo,
            uow_factory,
            clock,
        }
    }

    pub async fn execute(&self, command: CreateStaffCommand) -> Result<Staff, ApplicationError> {
        let mut staff = Staff::new(
            command.tenant_id,
            command.full_name,
            command.job_title,
            &*self.clock,
        );

        let mut uow = self.uow_factory.begin().await?;

        let event = StaffCreatedEvent {
            metadata: EventMetadata::new(
                "StaffCreated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            staff_id: staff.id,
            full_name: staff.full_name.clone(),
        };

        staff.raise_event(event);
        self.staff_repo.create(&staff, &mut *uow).await?;

        for domain_event in staff.take_events() {
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
        Ok(staff)
    }
}
