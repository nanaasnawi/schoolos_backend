use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Domain Events ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdated {
    pub progress_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub overall_progress: f64,
    pub tenant_id: Uuid,
    pub metadata: EventMetadata,
}

impl DomainEvent for ProgressUpdated {
    fn event_name(&self) -> &str {
        "learning.progress.updated"
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
pub struct StudentProgress {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub overall_progress: f64,
    pub lesson_completed: i32,
    pub lesson_total: i32,
    pub assignment_completed: i32,
    pub assignment_total: i32,
    pub quiz_completed: i32,
    pub quiz_total: i32,
    pub session_attended: i32,
    pub session_total: i32,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for StudentProgress {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            student_id: self.student_id,
            class_id: self.class_id,
            subject_id: self.subject_id,
            overall_progress: self.overall_progress,
            lesson_completed: self.lesson_completed,
            lesson_total: self.lesson_total,
            assignment_completed: self.assignment_completed,
            assignment_total: self.assignment_total,
            quiz_completed: self.quiz_completed,
            quiz_total: self.quiz_total,
            session_attended: self.session_attended,
            session_total: self.session_total,
            calculated_at: self.calculated_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for StudentProgress {
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

impl StudentProgress {
    pub fn new(tenant_id: Uuid, student_id: Uuid, class_id: Uuid, subject_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            student_id,
            class_id,
            subject_id,
            overall_progress: 0.0,
            lesson_completed: 0,
            lesson_total: 0,
            assignment_completed: 0,
            assignment_total: 0,
            quiz_completed: 0,
            quiz_total: 0,
            session_attended: 0,
            session_total: 0,
            calculated_at: now,
            created_at: now,
            updated_at: now,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn update(
        &mut self,
        lesson_completed: i32,
        lesson_total: i32,
        assignment_completed: i32,
        assignment_total: i32,
        quiz_completed: i32,
        quiz_total: i32,
        session_attended: i32,
        session_total: i32,
    ) {
        self.lesson_completed = lesson_completed;
        self.lesson_total = lesson_total;
        self.assignment_completed = assignment_completed;
        self.assignment_total = assignment_total;
        self.quiz_completed = quiz_completed;
        self.quiz_total = quiz_total;
        self.session_attended = session_attended;
        self.session_total = session_total;

        let total_items = lesson_total + assignment_total + quiz_total + session_total;
        let completed_items =
            lesson_completed + assignment_completed + quiz_completed + session_attended;

        self.overall_progress = if total_items > 0 {
            (completed_items as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };

        self.calculated_at = Utc::now();
        self.updated_at = Utc::now();
    }

    pub fn emit_updated(&mut self) {
        self.raise_event(ProgressUpdated {
            progress_id: self.id,
            student_id: self.student_id,
            class_id: self.class_id,
            subject_id: self.subject_id,
            overall_progress: self.overall_progress,
            tenant_id: self.tenant_id,
            metadata: EventMetadata::new(
                "learning.progress.updated".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                1,
                &crate::common::domain::clock::SystemClock,
            ),
        });
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
