use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct GradeLevel {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub level: i32,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,
    pub version: i32,
}

impl Clone for GradeLevel {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            level: self.level,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl PartialEq for GradeLevel {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.tenant_id == other.tenant_id
            && self.level == other.level
            && self.name == other.name
            && self.created_at == other.created_at
            && self.updated_at == other.updated_at
            && self.deleted_at == other.deleted_at
            && self.deleted_by == other.deleted_by
            && self.version == other.version
    }
}

impl Eq for GradeLevel {}

impl AggregateRoot for GradeLevel {
    fn id(&self) -> Uuid {
        self.id
    }

    fn version(&self) -> i32 {
        self.version
    }

    fn take_events(&mut self) -> Vec<Box<dyn crate::common::domain::event::DomainEvent>> {
        std::mem::take(&mut self.domain_events)
    }
}

impl GradeLevel {
    pub fn new(tenant_id: Uuid, level: i32, name: String, clock: &dyn Clock) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            level,
            name,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn raise_event(&mut self, event: impl crate::common::domain::event::DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
