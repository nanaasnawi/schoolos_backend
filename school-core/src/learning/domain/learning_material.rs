use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct LearningMaterial {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub material_type: String,
    pub title: String,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub order_index: i32,
    pub visibility: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for LearningMaterial {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            lesson_id: self.lesson_id,
            material_type: self.material_type.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            storage_key: self.storage_key.clone(),
            external_url: self.external_url.clone(),
            order_index: self.order_index,
            visibility: self.visibility.clone(),
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

impl AggregateRoot for LearningMaterial {
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

impl LearningMaterial {
    pub fn new(
        tenant_id: Uuid,
        lesson_id: Option<Uuid>,
        material_type: String,
        title: String,
        description: Option<String>,
        storage_key: Option<String>,
        external_url: Option<String>,
        order_index: i32,
        visibility: String,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!title.is_empty(), "title must not be empty");

        let now = clock.now();
        let id = Uuid::now_v7();

        let mut material = Self {
            id,
            tenant_id,
            lesson_id,
            material_type: material_type.clone(),
            title: title.clone(),
            description,
            storage_key,
            external_url,
            order_index,
            visibility,
            is_active: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        material.raise_event(
            crate::learning::domain::events::LearningMaterialCreatedEvent {
                metadata: crate::common::domain::event::EventMetadata::new(
                    "LearningMaterialCreated".to_string(),
                    tenant_id,
                    id.to_string(),
                    None,
                    None,
                    None,
                    1,
                    clock,
                ),
                material_id: id,
                title,
                material_type,
            },
        );

        material
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        lesson_id: Option<Uuid>,
        material_type: String,
        title: String,
        description: Option<String>,
        storage_key: Option<String>,
        external_url: Option<String>,
        order_index: i32,
        visibility: String,
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
            lesson_id,
            material_type,
            title,
            description,
            storage_key,
            external_url,
            order_index,
            visibility,
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
