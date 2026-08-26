use crate::common::error::ApplicationError;
use crate::notification::domain::notification_channel::NotificationChannel;
use crate::notification::domain::notification_preference::NotificationPreference;
use crate::notification::infrastructure::repository_traits::NotificationPreferenceRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct UpsertPreferenceCommand {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub channels: Vec<NotificationChannel>,
    pub is_enabled: bool,
}

pub struct UpsertPreferenceUseCase {
    repo: Arc<dyn NotificationPreferenceRepository>,
}

impl UpsertPreferenceUseCase {
    pub fn new(repo: Arc<dyn NotificationPreferenceRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        command: UpsertPreferenceCommand,
    ) -> Result<NotificationPreference, ApplicationError> {
        let existing = self
            .repo
            .find_by_user_and_type(
                command.user_id,
                command.tenant_id,
                &command.notification_type,
            )
            .await?;

        let pref = if let Some(mut p) = existing {
            p.update_channels(command.channels.clone());
            p.set_enabled(command.is_enabled);
            p
        } else {
            NotificationPreference::new(
                command.tenant_id,
                command.user_id,
                command.notification_type,
                command.channels.clone(),
            )
        };

        self.repo.upsert(&pref).await?;
        Ok(pref)
    }
}
