use crate::common::error::ApplicationError;
use crate::notification::infrastructure::repository_traits::NotificationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct MarkAllNotificationsReadCommand {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
}

pub struct MarkAllNotificationsReadUseCase {
    repo: Arc<dyn NotificationRepository>,
}

impl MarkAllNotificationsReadUseCase {
    pub fn new(repo: Arc<dyn NotificationRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        command: MarkAllNotificationsReadCommand,
    ) -> Result<(), ApplicationError> {
        self.repo
            .mark_all_read(command.user_id, command.tenant_id)
            .await?;
        Ok(())
    }
}
