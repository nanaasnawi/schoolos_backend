use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{FinalGradePublishedEvent, GradeCalculatedEvent, GradeEntryRecordedEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeEntry {
    pub id: Uuid,
    pub gradebook_id: Uuid,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub component_name: String,
    pub raw_score: f64,
    pub max_raw_score: f64,
    pub weight_percentage: f64,
    pub weighted_score: f64,
    pub recorded_at: DateTime<Utc>,
}

impl GradeEntry {
    pub fn new(
        gradebook_id: Uuid,
        source_type: String,
        source_id: Option<Uuid>,
        component_name: String,
        raw_score: f64,
        max_raw_score: f64,
        weight_percentage: f64,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        if gradebook_id.is_nil() {
            return Err(DomainError::Validation(
                "gradebook_id must not be nil".to_string(),
            ));
        }
        if max_raw_score <= 0.0 {
            return Err(DomainError::Validation(
                "max_raw_score must be > 0.0".to_string(),
            ));
        }
        if raw_score < 0.0 || raw_score > max_raw_score {
            return Err(DomainError::Validation(format!(
                "raw_score ({:.2}) must be between 0.0 and max_raw_score ({:.2})",
                raw_score, max_raw_score
            )));
        }

        let normalized_score = (raw_score / max_raw_score) * 100.0;
        let weighted_score = (normalized_score * weight_percentage) / 100.0;
        let now = clock.now();

        Ok(Self {
            id: Uuid::now_v7(),
            gradebook_id,
            source_type,
            source_id,
            component_name,
            raw_score,
            max_raw_score,
            weight_percentage,
            weighted_score,
            recorded_at: now,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GradeBook {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub final_score: Option<f64>,
    pub letter_grade: Option<String>,
    pub passed: Option<bool>,
    pub status: String,
    pub entries: Vec<GradeEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for GradeBook {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            student_id: self.student_id,
            class_id: self.class_id,
            subject_id: self.subject_id,
            academic_year_id: self.academic_year_id,
            final_score: self.final_score,
            letter_grade: self.letter_grade.clone(),
            passed: self.passed,
            status: self.status.clone(),
            entries: self.entries.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for GradeBook {
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

impl GradeBook {
    pub fn new(
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
        academic_year_id: Option<Uuid>,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        if tenant_id.is_nil() {
            return Err(DomainError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if student_id.is_nil() {
            return Err(DomainError::Validation(
                "student_id must not be nil".to_string(),
            ));
        }
        if class_id.is_nil() {
            return Err(DomainError::Validation(
                "class_id must not be nil".to_string(),
            ));
        }
        if subject_id.is_nil() {
            return Err(DomainError::Validation(
                "subject_id must not be nil".to_string(),
            ));
        }

        let now = clock.now();
        let id = Uuid::now_v7();

        Ok(Self {
            id,
            tenant_id,
            student_id,
            class_id,
            subject_id,
            academic_year_id,
            final_score: None,
            letter_grade: None,
            passed: None,
            status: "draft".to_string(),
            entries: Vec::new(),
            created_at: now,
            updated_at: now,
            domain_events: Vec::new(),
            version: 1,
        })
    }

    /// Record a GradeEntry into GradeBook
    pub fn record_grade(
        &mut self,
        source_type: String,
        source_id: Option<Uuid>,
        component_name: String,
        raw_score: f64,
        max_raw_score: f64,
        weight_percentage: f64,
        clock: &dyn Clock,
    ) -> Result<GradeEntry, DomainError> {
        if self.status == "published" {
            return Err(DomainError::Validation(
                "Cannot record grade in a published GradeBook".to_string(),
            ));
        }

        // Upsert entry for same source_id or component_name
        if let Some(pos) = self.entries.iter().position(|e| {
            (source_id.is_some() && e.source_id == source_id)
                || e.component_name.eq_ignore_ascii_case(&component_name)
        }) {
            self.entries.remove(pos);
        }

        let entry = GradeEntry::new(
            self.id,
            source_type,
            source_id,
            component_name,
            raw_score,
            max_raw_score,
            weight_percentage,
            clock,
        )?;

        self.entries.push(entry.clone());
        self.updated_at = clock.now();

        self.raise_event(GradeEntryRecordedEvent {
            metadata: EventMetadata::new(
                "GradeEntryRecorded".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            gradebook_id: self.id,
            student_id: self.student_id,
            score: raw_score,
        });

        Ok(entry)
    }

    /// Set calculated final grade results
    pub fn set_calculated_grade(
        &mut self,
        final_score: f64,
        letter_grade: String,
        passed: bool,
        clock: &dyn Clock,
    ) {
        self.final_score = Some(final_score);
        self.letter_grade = Some(letter_grade);
        self.passed = Some(passed);
        self.status = "calculated".to_string();
        self.updated_at = clock.now();

        self.raise_event(GradeCalculatedEvent {
            metadata: EventMetadata::new(
                "GradeCalculated".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            gradebook_id: self.id,
            student_id: self.student_id,
            final_score,
            passed,
        });
    }

    pub fn publish(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        if self.final_score.is_none() {
            return Err(DomainError::Validation(
                "Cannot publish gradebook before final score is calculated".to_string(),
            ));
        }

        self.status = "published".to_string();
        self.updated_at = clock.now();

        self.raise_event(FinalGradePublishedEvent {
            metadata: EventMetadata::new(
                "FinalGradePublished".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            gradebook_id: self.id,
            student_id: self.student_id,
        });

        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
