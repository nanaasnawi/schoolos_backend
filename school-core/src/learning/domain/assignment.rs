use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{
    AssignmentArchivedEvent, AssignmentClosedEvent, AssignmentCreatedEvent,
    AssignmentPublishedEvent, AssignmentUpdatedEvent,
};
pub use super::events::{AssignmentSubmitted, GradeReleased};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssignmentStatus {
    Draft,
    Published,
    Closed,
    Archived,
}

impl AssignmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Closed => "closed",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "published" => Self::Published,
            "closed" => Self::Closed,
            "archived" => Self::Archived,
            _ => Self::Draft,
        }
    }
}

impl std::fmt::Display for AssignmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Assignment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: i32,
    pub due_at: Option<DateTime<Utc>>,
    pub assignment_type: String,
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

impl Clone for Assignment {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            lesson_id: self.lesson_id,
            title: self.title.clone(),
            description: self.description.clone(),
            instructions: self.instructions.clone(),
            max_score: self.max_score,
            due_at: self.due_at,
            assignment_type: self.assignment_type.clone(),
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

impl AggregateRoot for Assignment {
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

impl Assignment {
    pub fn new(
        tenant_id: Uuid,
        lesson_id: Uuid,
        title: String,
        description: Option<String>,
        instructions: Option<String>,
        max_score: i32,
        due_at: Option<DateTime<Utc>>,
        assignment_type: String,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!lesson_id.is_nil(), "lesson_id must not be nil");
        if title.is_empty() {
            return Err(DomainError::Validation(
                "title must not be empty".to_string(),
            ));
        }

        let now = clock.now();
        if let Some(due) = due_at {
            if due <= now {
                return Err(DomainError::Validation(
                    "due_date must be in the future".to_string(),
                ));
            }
        }

        let id = Uuid::now_v7();
        let mut assignment = Self {
            id,
            tenant_id,
            lesson_id,
            title: title.clone(),
            description,
            instructions,
            max_score,
            due_at,
            assignment_type,
            status: "draft".to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        assignment.raise_event(AssignmentCreatedEvent {
            metadata: EventMetadata::new(
                "AssignmentCreated".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            assignment_id: id,
            lesson_id,
            title,
        });

        Ok(assignment)
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        lesson_id: Uuid,
        title: String,
        description: Option<String>,
        instructions: Option<String>,
        max_score: i32,
        due_at: Option<DateTime<Utc>>,
        assignment_type: String,
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
            lesson_id,
            title,
            description,
            instructions,
            max_score,
            due_at,
            assignment_type,
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

    /// Domain Invariant: Publish Assignment
    /// Rules:
    /// - Assignment must be in Draft status
    /// - Parent Lesson MUST be Published (cannot publish assignment for draft lesson)
    /// - Title must not be empty
    /// - Due date (if set) must be in the future
    pub fn publish(&mut self, lesson_status: &str, clock: &dyn Clock) -> Result<(), DomainError> {
        if self.status != "draft" {
            return Err(DomainError::Validation(format!(
                "Cannot publish assignment in '{}' state",
                self.status
            )));
        }
        if lesson_status != "published" {
            return Err(DomainError::Validation(
                "Cannot publish assignment when associated lesson is not published".to_string(),
            ));
        }
        if self.title.is_empty() {
            return Err(DomainError::Validation(
                "Assignment title cannot be empty".to_string(),
            ));
        }
        if let Some(due) = self.due_at {
            if due <= clock.now() {
                return Err(DomainError::Validation(
                    "due_date must be in the future".to_string(),
                ));
            }
        }

        self.status = "published".to_string();
        self.updated_at = clock.now();

        self.raise_event(AssignmentPublishedEvent {
            metadata: EventMetadata::new(
                "AssignmentPublished".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            assignment_id: self.id,
            lesson_id: self.lesson_id,
            title: self.title.clone(),
        });

        Ok(())
    }

    /// Domain Invariant: Close Assignment
    pub fn close(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        if self.status != "published" {
            return Err(DomainError::Validation(format!(
                "Cannot close assignment in '{}' state",
                self.status
            )));
        }

        self.status = "closed".to_string();
        self.updated_at = clock.now();

        self.raise_event(AssignmentClosedEvent {
            metadata: EventMetadata::new(
                "AssignmentClosed".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            assignment_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    /// Domain Invariant: Archive Assignment
    pub fn archive(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "archived".to_string();
        self.updated_at = clock.now();

        self.raise_event(AssignmentArchivedEvent {
            metadata: EventMetadata::new(
                "AssignmentArchived".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            assignment_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    /// Domain Invariant: Update Assignment Details
    /// Rules: Closed or Archived assignments cannot be modified.
    pub fn update(
        &mut self,
        title: Option<String>,
        description: Option<String>,
        instructions: Option<String>,
        max_score: Option<i32>,
        due_at: Option<DateTime<Utc>>,
        clock: &dyn Clock,
    ) -> Result<(), DomainError> {
        if self.status == "closed" || self.status == "archived" {
            return Err(DomainError::Validation(format!(
                "Cannot modify assignment in '{}' state",
                self.status
            )));
        }

        if let Some(t) = title {
            if !t.is_empty() {
                self.title = t;
            }
        }
        if let Some(d) = description {
            self.description = Some(d);
        }
        if let Some(inst) = instructions {
            self.instructions = Some(inst);
        }
        if let Some(score) = max_score {
            if score > 0 {
                self.max_score = score;
            }
        }
        if let Some(due) = due_at {
            if due <= clock.now() {
                return Err(DomainError::Validation(
                    "due_date must be in the future".to_string(),
                ));
            }
            self.due_at = Some(due);
        }

        self.updated_at = clock.now();

        self.raise_event(AssignmentUpdatedEvent {
            metadata: EventMetadata::new(
                "AssignmentUpdated".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            assignment_id: self.id,
            title: self.title.clone(),
        });

        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
