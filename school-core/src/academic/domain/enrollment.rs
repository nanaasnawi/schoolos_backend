use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Enrollment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub academic_year_id: Uuid,
    pub status: String,
    pub enrolled_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,
    pub version: i32,
}

impl AggregateRoot for Enrollment {
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

impl Enrollment {
    pub fn new(
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
        academic_year_id: Uuid,
        clock: &dyn Clock,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            student_id,
            class_id,
            academic_year_id,
            status: "Active".to_string(),
            enrolled_at: clock.now(),
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn raise_event(&mut self, event: impl crate::common::domain::event::DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn close(&mut self) {
        self.status = "Closed".to_string();
    }
}
