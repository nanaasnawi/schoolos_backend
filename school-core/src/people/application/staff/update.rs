use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::StaffUpdatedEvent;
use crate::people::domain::staff::Staff;
use crate::people::infrastructure::repository_traits::StaffRepository;

#[derive(Debug, Clone)]
pub struct UpdateStaffCommand {
    pub tenant_id: Uuid,
    pub staff_id: Uuid,
    pub full_name: Option<String>,
    pub job_title: Option<String>,
    pub request_id: Option<String>,
}

pub struct UpdateStaffUseCase {
    staff_repo: Arc<dyn StaffRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl UpdateStaffUseCase {
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

    pub async fn execute(&self, command: UpdateStaffCommand) -> Result<Staff, ApplicationError> {
        let mut staff = self.staff_repo.find_by_id(command.staff_id).await?.ok_or(
            ApplicationError::NotFound(
                ErrorCode::StaffNotFound,
                format!("Staff not found: {}", command.staff_id),
            ),
        )?;

        if staff.tenant_id != command.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Staff does not belong to the tenant".to_string(),
            ));
        }

        if let Some(full_name) = command.full_name {
            staff.full_name = full_name;
        }
        if let Some(job_title) = command.job_title {
            staff.job_title = job_title;
        }
        staff.updated_at = self.clock.now();

        let mut uow = self.uow_factory.begin().await?;
        self.staff_repo.update(&staff, &mut *uow).await?;

        let event = StaffUpdatedEvent {
            metadata: EventMetadata::new(
                "StaffUpdated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            staff_id: staff.id,
        };

        let outbox_event = OutboxEvent {
            id: Uuid::now_v7(),
            event_type: event.metadata.event_type.clone(),
            payload: serde_json::to_value(&event)
                .map_err(|e| ApplicationError::Internal(e.to_string()))?,
            status: crate::common::domain::outbox::OutboxStatus::Pending,
            created_at: self.clock.now(),
            processed_at: None,
        };

        self.outbox_repo.save(&outbox_event, &mut *uow).await?;
        uow.commit().await?;

        Ok(staff)
    }
}
