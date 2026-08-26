use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Curriculum {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Curriculum {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            code: self.code.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for Curriculum {
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

impl Curriculum {
    pub fn new(
        tenant_id: Uuid,
        code: String,
        name: String,
        description: Option<String>,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!code.is_empty(), "code must not be empty");
        assert!(!name.is_empty(), "name must not be empty");

        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            code,
            name,
            description,
            is_active: true,
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
        code: String,
        name: String,
        description: Option<String>,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        deleted_by: Option<Uuid>,
        version: i32,
    ) -> Self {
        Self {
            id,
            tenant_id,
            code,
            name,
            description,
            is_active,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version,
        }
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
