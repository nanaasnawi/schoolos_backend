use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyllabusCompetency {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub competency_type: String,
    pub description: String,
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for SyllabusCompetency {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            syllabus_id: self.syllabus_id,
            code: self.code.clone(),
            competency_type: self.competency_type.clone(),
            description: self.description.clone(),
            order_index: self.order_index,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl SyllabusCompetency {
    pub fn new(
        tenant_id: Uuid,
        syllabus_id: Uuid,
        code: String,
        competency_type: String,
        description: String,
        order_index: i32,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!syllabus_id.is_nil(), "syllabus_id must not be nil");
        assert!(!code.is_empty(), "code must not be empty");
        assert!(!description.is_empty(), "description must not be empty");

        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            syllabus_id,
            code,
            competency_type,
            description,
            order_index,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        syllabus_id: Uuid,
        code: String,
        competency_type: String,
        description: String,
        order_index: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        version: i32,
    ) -> Self {
        Self {
            id,
            tenant_id,
            syllabus_id,
            code,
            competency_type,
            description,
            order_index,
            created_at,
            updated_at,
            deleted_at,
            domain_events: Vec::new(),
            version,
        }
    }
}
