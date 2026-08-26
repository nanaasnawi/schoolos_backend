use crate::common::error::ApplicationError;
use crate::notification::infrastructure::repository_traits::NotificationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct MarkNotificationReadCommand {
    pub notification_id: Uuid,
    pub user_id: Uuid,
}

pub struct MarkNotificationReadUseCase {
    repo: Arc<dyn NotificationRepository>,
}

impl MarkNotificationReadUseCase {
    pub fn new(repo: Arc<dyn NotificationRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        command: MarkNotificationReadCommand,
    ) -> Result<(), ApplicationError> {
        self.repo
            .mark_read(command.notification_id, command.user_id)
            .await?;
        Ok(())
    }
}
