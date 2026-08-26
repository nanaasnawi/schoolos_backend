use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Domain Events ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonStarted {
    pub session_id: Uuid,
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub tenant_id: Uuid,
    pub metadata: EventMetadata,
}

impl DomainEvent for LessonStarted {
    fn event_name(&self) -> &str {
        "learning.lesson.started"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonCompleted {
    pub session_id: Uuid,
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub tenant_id: Uuid,
    pub duration_minutes: i64,
    pub metadata: EventMetadata,
}

impl DomainEvent for LessonCompleted {
    fn event_name(&self) -> &str {
        "learning.lesson.completed"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentJoinedSession {
    pub attendance_id: Uuid,
    pub session_id: Uuid,
    pub student_id: Uuid,
    pub tenant_id: Uuid,
    pub metadata: EventMetadata,
}

impl DomainEvent for StudentJoinedSession {
    fn event_name(&self) -> &str {
        "learning.session.student_joined"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ── Aggregate ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct LearningSession {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for LearningSession {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            lesson_id: self.lesson_id,
            class_id: self.class_id,
            teacher_id: self.teacher_id,
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            ended_at: self.ended_at,
            status: self.status.clone(),
            notes: self.notes.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for LearningSession {
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

impl LearningSession {
    pub fn start_new(
        tenant_id: Uuid,
        lesson_id: Uuid,
        class_id: Uuid,
        teacher_id: Uuid,
        notes: Option<String>,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!lesson_id.is_nil(), "lesson_id must not be nil");
        assert!(!class_id.is_nil(), "class_id must not be nil");
        assert!(!teacher_id.is_nil(), "teacher_id must not be nil");

        let now = clock.now();
        let mut s = Self {
            id: Uuid::now_v7(),
            tenant_id,
            lesson_id,
            class_id,
            teacher_id,
            scheduled_at: None,
            started_at: Some(now),
            ended_at: None,
            status: "active".to_string(),
            notes,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        s.raise_event(LessonStarted {
            session_id: s.id,
            lesson_id: s.lesson_id,
            class_id: s.class_id,
            teacher_id: s.teacher_id,
            tenant_id: s.tenant_id,
            metadata: EventMetadata::new(
                "learning.lesson.started".to_string(),
                tenant_id,
                Uuid::now_v7().to_string(),
                None,
                None,
                Some(teacher_id),
                1,
                clock,
            ),
        });

        s
    }

    pub fn end(&mut self, clock: &dyn Clock) {
        assert_eq!(self.status, "active", "can only end an active session");
        let now = clock.now();
        self.ended_at = Some(now);
        self.status = "completed".to_string();
        self.updated_at = now;

        let duration = self
            .started_at
            .map(|s| (now - s).num_minutes())
            .unwrap_or(0);

        self.raise_event(LessonCompleted {
            session_id: self.id,
            lesson_id: self.lesson_id,
            class_id: self.class_id,
            teacher_id: self.teacher_id,
            tenant_id: self.tenant_id,
            duration_minutes: duration,
            metadata: EventMetadata::new(
                "learning.lesson.completed".to_string(),
                self.tenant_id,
                Uuid::now_v7().to_string(),
                None,
                None,
                Some(self.teacher_id),
                1,
                clock,
            ),
        });
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        lesson_id: Uuid,
        class_id: Uuid,
        teacher_id: Uuid,
        scheduled_at: Option<DateTime<Utc>>,
        started_at: Option<DateTime<Utc>>,
        ended_at: Option<DateTime<Utc>>,
        status: String,
        notes: Option<String>,
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
            class_id,
            teacher_id,
            scheduled_at,
            started_at,
            ended_at,
            status,
            notes,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version,
        }
    }
}
