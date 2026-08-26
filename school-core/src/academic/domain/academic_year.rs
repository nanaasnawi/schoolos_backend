use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AcademicYear {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String, // e.g., "2024/2025"
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,
    pub version: i32,
}

impl AggregateRoot for AcademicYear {
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

impl AcademicYear {
    pub fn new(
        tenant_id: Uuid,
        name: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
        clock: &dyn Clock,
    ) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            name,
            start_date,
            end_date,
            is_active: true,
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
