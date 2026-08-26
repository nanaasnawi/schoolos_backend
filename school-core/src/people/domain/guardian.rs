use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Guardian {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub full_name: String,
    pub phone_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Guardian {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            full_name: self.full_name.clone(),
            phone_number: self.phone_number.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for Guardian {
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

impl Guardian {
    pub fn new(
        tenant_id: Uuid,
        full_name: String,
        phone_number: Option<String>,
        clock: &dyn Clock,
    ) -> Self {
        if tenant_id.is_nil() {
            panic!("Guardian::new called with nil tenant_id");
        }
        if full_name.trim().is_empty() {
            panic!("Guardian::new called with empty full_name");
        }
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            user_id: None,
            full_name,
            phone_number,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        full_name: String,
        phone_number: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        deleted_by: Option<Uuid>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            user_id,
            full_name,
            phone_number,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn link_user(&mut self, user_id: Uuid, clock: &dyn Clock) {
        self.user_id = Some(user_id);
        self.updated_at = clock.now();
    }

    pub fn update_profile(
        &mut self,
        full_name: String,
        phone_number: Option<String>,
        clock: &dyn Clock,
    ) {
        self.full_name = full_name;
        self.phone_number = phone_number;
        self.updated_at = clock.now();
    }
}
