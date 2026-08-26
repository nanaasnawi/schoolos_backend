use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Class {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub homeroom_teacher_id: Option<Uuid>,
    pub name: String, // e.g., "Kelas 1A"
    pub capacity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,
    pub version: i32,
}

impl AggregateRoot for Class {
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

impl Class {
    pub fn new(
        tenant_id: Uuid,
        academic_year_id: Uuid,
        grade_level_id: Uuid,
        name: String,
        capacity: i32,
        clock: &dyn Clock,
    ) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            academic_year_id,
            grade_level_id,
            homeroom_teacher_id: None,
            name,
            capacity,
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

    pub fn assign_homeroom_teacher(&mut self, teacher_id: Uuid, clock: &dyn Clock) {
        self.homeroom_teacher_id = Some(teacher_id);
        self.updated_at = clock.now();
    }
}
