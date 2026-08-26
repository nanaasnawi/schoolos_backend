use school_core::notification::domain::notification_preference::NotificationPreference;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct PreferenceResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub channels: Vec<String>,
    pub is_enabled: bool,
}

impl From<NotificationPreference> for PreferenceResponse {
    fn from(p: NotificationPreference) -> Self {
        Self {
            id: p.id,
            user_id: p.user_id,
            notification_type: p.notification_type,
            channels: p.channels.iter().map(|c| c.as_str().to_string()).collect(),
            is_enabled: p.is_enabled,
        }
    }
}
