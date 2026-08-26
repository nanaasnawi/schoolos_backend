use std::sync::Arc;

use uuid::Uuid;

use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository};
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::infrastructure::uow::UnitOfWorkFactory;
use crate::people::domain::events::TeacherUpdatedEvent;
use crate::people::domain::teacher::Teacher;
use crate::people::infrastructure::repository_traits::TeacherRepository;

#[derive(Debug, Clone)]
pub struct UpdateTeacherCommand {
    pub tenant_id: Uuid,
    pub teacher_id: Uuid,
    pub full_name: Option<String>,
    pub nip: Option<String>,
    pub request_id: Option<String>,
}

pub struct UpdateTeacherUseCase {
    teacher_repo: Arc<dyn TeacherRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    uow_factory: Arc<dyn UnitOfWorkFactory>,
    clock: Arc<dyn Clock>,
}

impl UpdateTeacherUseCase {
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
        command: UpdateTeacherCommand,
    ) -> Result<Teacher, ApplicationError> {
        let mut teacher = self
            .teacher_repo
            .find_by_id(command.teacher_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                ErrorCode::TeacherNotFound,
                format!("Teacher not found: {}", command.teacher_id),
            ))?;

        if teacher.tenant_id != command.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Teacher does not belong to the tenant".to_string(),
            ));
        }

        if let Some(nip) = command.nip {
            teacher.nip = Some(nip);
        }
        if let Some(full_name) = command.full_name {
            teacher.full_name = full_name;
        }
        teacher.updated_at = self.clock.now();

        let mut uow = self.uow_factory.begin().await?;
        self.teacher_repo.update(&teacher, &mut *uow).await?;

        let event = TeacherUpdatedEvent {
            metadata: EventMetadata::new(
                "TeacherUpdated".to_string(),
                command.tenant_id,
                command.request_id.clone().unwrap_or_default(),
                command.request_id.clone(),
                None,
                None,
                1,
                &*self.clock,
            ),
            teacher_id: teacher.id,
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

        Ok(teacher)
    }
}
