use chrono::{DateTime, Utc};
use school_core::notification::domain::notification::Notification;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
    pub notification_type: String,
    pub channel: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<Notification> for NotificationResponse {
    fn from(n: Notification) -> Self {
        Self {
            id: n.id,
            user_id: n.user_id,
            title: n.title,
            body: n.body,
            notification_type: n.notification_type,
            channel: n.channel.as_str().to_string(),
            reference_type: n.reference_type,
            reference_id: n.reference_id,
            is_read: n.is_read,
            read_at: n.read_at,
            created_at: n.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadCountResponse {
    pub count: i64,
}
