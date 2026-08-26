use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    #[sqlx(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,
    pub version: i32,
}

impl AggregateRoot for Tenant {
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

impl Tenant {
    pub fn new(name: String, domain: Option<String>, clock: &dyn Clock) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            name,
            domain,
            is_active: true,
            created_at: now,
            updated_at: now,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn raise_event(&mut self, event: impl crate::common::domain::event::DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
