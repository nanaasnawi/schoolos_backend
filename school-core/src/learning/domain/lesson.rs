use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{
    LessonArchivedEvent, LessonCreatedEvent, LessonPublishedEvent, LessonUpdatedEvent,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LessonStatus {
    Draft,
    Published,
    Archived,
}

impl LessonStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "published" => Self::Published,
            "archived" => Self::Archived,
            _ => Self::Draft,
        }
    }
}

impl std::fmt::Display for LessonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lesson {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: i32,
    pub order_index: i32,
    pub status: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Lesson {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            syllabus_id: self.syllabus_id,
            code: self.code.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            learning_objectives: self.learning_objectives.clone(),
            duration_minutes: self.duration_minutes,
            order_index: self.order_index,
            status: self.status.clone(),
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

impl AggregateRoot for Lesson {
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

impl Lesson {
    pub fn new(
        tenant_id: Uuid,
        syllabus_id: Uuid,
        code: String,
        title: String,
        description: Option<String>,
        learning_objectives: Option<String>,
        duration_minutes: i32,
        order_index: i32,
        status: String,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!syllabus_id.is_nil(), "syllabus_id must not be nil");
        assert!(!code.is_empty(), "code must not be empty");
        assert!(!title.is_empty(), "title must not be empty");

        let now = clock.now();
        let id = Uuid::now_v7();

        let mut lesson = Self {
            id,
            tenant_id,
            syllabus_id,
            code: code.clone(),
            title: title.clone(),
            description,
            learning_objectives,
            duration_minutes,
            order_index,
            status,
            is_active: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        lesson.raise_event(LessonCreatedEvent {
            metadata: EventMetadata::new(
                "LessonCreated".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            lesson_id: id,
            syllabus_id,
            code,
            title,
        });

        lesson
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        syllabus_id: Uuid,
        code: String,
        title: String,
        description: Option<String>,
        learning_objectives: Option<String>,
        duration_minutes: i32,
        order_index: i32,
        status: String,
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
            syllabus_id,
            code,
            title,
            description,
            learning_objectives,
            duration_minutes,
            order_index,
            status,
            is_active,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version,
        }
    }

    /// Domain Business Invariant: Publish Lesson
    /// Rule: Lesson cannot be published without at least 1 learning material.
    pub fn publish(&mut self, material_count: usize, clock: &dyn Clock) -> Result<(), DomainError> {
        if self.status == "archived" {
            return Err(DomainError::Validation(
                "Cannot publish an archived lesson".to_string(),
            ));
        }
        if material_count == 0 {
            return Err(DomainError::Validation(
                "Cannot publish lesson without at least one material".to_string(),
            ));
        }

        self.status = "published".to_string();
        self.updated_at = clock.now();

        self.raise_event(LessonPublishedEvent {
            metadata: EventMetadata::new(
                "LessonPublished".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            lesson_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    /// Domain Business Invariant: Archive Lesson
    pub fn archive(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "archived".to_string();
        self.updated_at = clock.now();

        self.raise_event(LessonArchivedEvent {
            metadata: EventMetadata::new(
                "LessonArchived".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            lesson_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    /// Domain Business Invariant: Update Lesson Details
    /// Rule: Cannot modify lesson after it has been archived.
    pub fn update(
        &mut self,
        title: Option<String>,
        description: Option<String>,
        learning_objectives: Option<String>,
        duration_minutes: Option<i32>,
        clock: &dyn Clock,
    ) -> Result<(), DomainError> {
        if self.status == "archived" {
            return Err(DomainError::Validation(
                "Cannot modify an archived lesson".to_string(),
            ));
        }

        if let Some(t) = title {
            if !t.is_empty() {
                self.title = t;
            }
        }
        if let Some(d) = description {
            self.description = Some(d);
        }
        if let Some(obj) = learning_objectives {
            self.learning_objectives = Some(obj);
        }
        if let Some(dur) = duration_minutes {
            if dur > 0 {
                self.duration_minutes = dur;
            }
        }

        self.updated_at = clock.now();

        self.raise_event(LessonUpdatedEvent {
            metadata: EventMetadata::new(
                "LessonUpdated".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            lesson_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
