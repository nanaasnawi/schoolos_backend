use crate::common::error::InfrastructureError;
use crate::notification::domain::notification::Notification;
use crate::notification::domain::notification_preference::NotificationPreference;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn create(&self, notification: &Notification) -> Result<(), InfrastructureError>;
    async fn find_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Notification>, InfrastructureError>;
    async fn count_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError>;
    async fn count_unread(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError>;
    async fn mark_read(&self, id: Uuid, user_id: Uuid) -> Result<(), InfrastructureError>;
    async fn mark_all_read(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait NotificationPreferenceRepository: Send + Sync {
    async fn upsert(&self, pref: &NotificationPreference) -> Result<(), InfrastructureError>;
    async fn find_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<NotificationPreference>, InfrastructureError>;
    async fn find_by_user_and_type(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationPreference>, InfrastructureError>;
}
