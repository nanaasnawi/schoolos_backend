use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::event::DomainEvent;
use crate::notification::domain::notification_channel::NotificationChannel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationPreference {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub channels: Vec<NotificationChannel>,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for NotificationPreference {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            notification_type: self.notification_type.clone(),
            channels: self.channels.clone(),
            is_enabled: self.is_enabled,
            created_at: self.created_at,
            updated_at: self.updated_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for NotificationPreference {
    fn id(&self) -> Uuid {
        self.id
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.domain_events)
    }
}

impl NotificationPreference {
    pub fn new(
        tenant_id: Uuid,
        user_id: Uuid,
        notification_type: String,
        channels: Vec<NotificationChannel>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            user_id,
            notification_type,
            channels,
            is_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn update_channels(&mut self, channels: Vec<NotificationChannel>) {
        self.channels = channels;
        self.updated_at = Utc::now();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
        self.updated_at = Utc::now();
    }
}
