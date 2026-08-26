use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::event::DomainEvent;
use crate::notification::domain::notification_channel::NotificationChannel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
    pub notification_type: String,
    pub channel: NotificationChannel,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Notification {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            title: self.title.clone(),
            body: self.body.clone(),
            notification_type: self.notification_type.clone(),
            channel: self.channel.clone(),
            reference_type: self.reference_type.clone(),
            reference_id: self.reference_id,
            is_read: self.is_read,
            read_at: self.read_at,
            created_at: self.created_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for Notification {
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

impl Notification {
    pub fn new(
        tenant_id: Uuid,
        user_id: Uuid,
        title: String,
        body: String,
        notification_type: String,
        channel: NotificationChannel,
        reference_type: Option<String>,
        reference_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            user_id,
            title,
            body,
            notification_type,
            channel,
            reference_type,
            reference_id,
            is_read: false,
            read_at: None,
            created_at: Utc::now(),
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn mark_read(&mut self) {
        self.is_read = true;
        self.read_at = Some(Utc::now());
    }
}
