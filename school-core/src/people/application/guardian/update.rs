use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::GuardianUpdatedEvent;
use crate::people::domain::guardian::Guardian;
use crate::people::infrastructure::repository_traits::GuardianRepository;

#[derive(Debug, Clone)]
pub struct UpdateGuardianCommand {
    pub tenant_id: Uuid,
    pub guardian_id: Uuid,
    pub full_name: Option<String>,
    pub phone_number: Option<String>,
    pub request_id: Option<String>,
}

pub struct UpdateGuardianUseCase {
    guardian_repo: Arc<dyn GuardianRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl UpdateGuardianUseCase {
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
        command: UpdateGuardianCommand,
    ) -> Result<Guardian, ApplicationError> {
        let mut guardian = self
            .guardian_repo
            .find_by_id(command.guardian_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                ErrorCode::GuardianNotFound,
                format!("Guardian not found: {}", command.guardian_id),
            ))?;

        if guardian.tenant_id != command.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Guardian does not belong to the tenant".to_string(),
            ));
        }

        if let Some(phone_number) = command.phone_number {
            guardian.phone_number = Some(phone_number);
        }
        if let Some(full_name) = command.full_name {
            guardian.full_name = full_name;
        }
        guardian.updated_at = self.clock.now();

        let mut uow = self.uow_factory.begin().await?;
        self.guardian_repo.update(&guardian, &mut *uow).await?;

        let event = GuardianUpdatedEvent {
            metadata: EventMetadata::new(
                "GuardianUpdated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            guardian_id: guardian.id,
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

        Ok(guardian)
    }
}
