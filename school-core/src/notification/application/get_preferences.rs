use crate::common::error::ApplicationError;
use crate::notification::domain::notification_preference::NotificationPreference;
use crate::notification::infrastructure::repository_traits::NotificationPreferenceRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetPreferencesQuery {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
}

pub struct GetPreferencesUseCase {
    repo: Arc<dyn NotificationPreferenceRepository>,
}

impl GetPreferencesUseCase {
    pub fn new(repo: Arc<dyn NotificationPreferenceRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetPreferencesQuery,
    ) -> Result<Vec<NotificationPreference>, ApplicationError> {
        Ok(self
            .repo
            .find_by_user(query.user_id, query.tenant_id)
            .await?)
    }
}
