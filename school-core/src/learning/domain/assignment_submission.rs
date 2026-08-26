use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{
    SubmissionAttemptAddedEvent, SubmissionCreatedEvent, SubmissionGradedEvent,
    SubmissionGradingStartedEvent, SubmissionReturnedEvent,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubmissionStatus {
    Draft,
    Submitted,
    Grading,
    Graded,
    Returned,
    Late,
}

impl SubmissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Grading => "grading",
            Self::Graded => "graded",
            Self::Returned => "returned",
            Self::Late => "late",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "submitted" => Self::Submitted,
            "grading" => Self::Grading,
            "graded" => Self::Graded,
            "returned" => Self::Returned,
            "late" => Self::Late,
            _ => Self::Draft,
        }
    }
}

impl std::fmt::Display for SubmissionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionAttempt {
    pub id: Uuid,
    pub submission_id: Uuid,
    pub attempt_number: i32,
    pub content: Option<String>,
    pub file_url: Option<String>,
    pub checksum: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub is_late: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignmentSubmission {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub assignment_id: Uuid,
    pub student_id: Uuid,
    pub content: Option<String>,
    pub file_url: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub status: String,
    pub score: Option<i32>,
    pub feedback: Option<String>,
    pub graded_at: Option<DateTime<Utc>>,
    pub graded_by: Option<Uuid>,
    pub attempts: Vec<SubmissionAttempt>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for AssignmentSubmission {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            assignment_id: self.assignment_id,
            student_id: self.student_id,
            content: self.content.clone(),
            file_url: self.file_url.clone(),
            submitted_at: self.submitted_at,
            status: self.status.clone(),
            score: self.score,
            feedback: self.feedback.clone(),
            graded_at: self.graded_at,
            graded_by: self.graded_by,
            attempts: self.attempts.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for AssignmentSubmission {
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

impl AssignmentSubmission {
    pub fn new(
        tenant_id: Uuid,
        assignment_id: Uuid,
        student_id: Uuid,
        content: Option<String>,
        file_url: Option<String>,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!assignment_id.is_nil(), "assignment_id must not be nil");
        assert!(!student_id.is_nil(), "student_id must not be nil");

        let now = clock.now();
        let id = Uuid::now_v7();

        let initial_attempt = SubmissionAttempt {
            id: Uuid::now_v7(),
            submission_id: id,
            attempt_number: 1,
            content: content.clone(),
            file_url: file_url.clone(),
            checksum: None,
            submitted_at: now,
            is_late: false,
        };

        let mut submission = Self {
            id,
            tenant_id,
            assignment_id,
            student_id,
            content,
            file_url,
            submitted_at: now,
            status: "submitted".to_string(),
            score: None,
            feedback: None,
            graded_at: None,
            graded_by: None,
            attempts: vec![initial_attempt],
            created_at: now,
            updated_at: now,
            domain_events: Vec::new(),
            version: 1,
        };

        submission.raise_event(SubmissionCreatedEvent {
            metadata: EventMetadata::new(
                "SubmissionCreated".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            submission_id: id,
            assignment_id,
            student_id,
        });

        submission
    }

    /// Domain Business Invariant: Add Submission Attempt
    /// Rules:
    /// - Cannot submit if assignment status is not 'published' (e.g. 'draft', 'closed', 'archived')
    /// - Cannot resubmit if submission has already been graded or returned
    /// - Cannot exceed max attempts limit
    /// - Automatically marks as late if submitted_at > due_at
    pub fn add_attempt(
        &mut self,
        content: Option<String>,
        file_url: Option<String>,
        checksum: Option<String>,
        assignment_status: &str,
        due_at: Option<DateTime<Utc>>,
        max_attempts: usize,
        clock: &dyn Clock,
    ) -> Result<SubmissionAttempt, DomainError> {
        if assignment_status != "published" {
            return Err(DomainError::Validation(format!(
                "Cannot submit attempt when assignment status is '{}'",
                assignment_status
            )));
        }

        if self.status == "graded" || self.status == "returned" {
            return Err(DomainError::Validation(
                "Cannot add attempt to a graded or returned submission".to_string(),
            ));
        }

        if self.attempts.len() >= max_attempts {
            return Err(DomainError::Validation(format!(
                "Maximum attempt limit ({}) reached for this submission",
                max_attempts
            )));
        }

        let now = clock.now();
        let is_late = due_at.is_some_and(|due| now > due);
        let attempt_number = (self.attempts.len() + 1) as i32;

        let attempt = SubmissionAttempt {
            id: Uuid::now_v7(),
            submission_id: self.id,
            attempt_number,
            content: content.clone(),
            file_url: file_url.clone(),
            checksum,
            submitted_at: now,
            is_late,
        };

        self.attempts.push(attempt.clone());
        self.content = content;
        self.file_url = file_url;
        self.submitted_at = now;
        self.status = if is_late {
            "late".to_string()
        } else {
            "submitted".to_string()
        };
        self.updated_at = now;

        self.raise_event(SubmissionAttemptAddedEvent {
            metadata: EventMetadata::new(
                "SubmissionAttemptAdded".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            submission_id: self.id,
            attempt_number,
            is_late,
        });

        Ok(attempt)
    }

    /// Domain Invariant: Start Grading
    pub fn start_grading(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "grading".to_string();
        self.updated_at = clock.now();

        self.raise_event(SubmissionGradingStartedEvent {
            metadata: EventMetadata::new(
                "SubmissionGradingStarted".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            submission_id: self.id,
        });

        Ok(())
    }

    /// Domain Invariant: Grade Submission
    /// Rule: Score must be between 0 and max_score
    pub fn grade(
        &mut self,
        score: i32,
        max_score: i32,
        feedback: Option<String>,
        graded_by: Uuid,
        clock: &dyn Clock,
    ) -> Result<(), DomainError> {
        if score < 0 || score > max_score {
            return Err(DomainError::Validation(format!(
                "Score {} must be between 0 and max score {}",
                score, max_score
            )));
        }

        let now = clock.now();
        self.score = Some(score);
        self.feedback = feedback;
        self.graded_by = Some(graded_by);
        self.graded_at = Some(now);
        self.status = "graded".to_string();
        self.updated_at = now;

        self.raise_event(SubmissionGradedEvent {
            metadata: EventMetadata::new(
                "SubmissionGraded".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            submission_id: self.id,
            score,
            graded_by,
        });

        Ok(())
    }

    /// Domain Invariant: Return Submission to Student
    pub fn return_submission(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "returned".to_string();
        self.updated_at = clock.now();

        self.raise_event(SubmissionReturnedEvent {
            metadata: EventMetadata::new(
                "SubmissionReturned".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            submission_id: self.id,
        });

        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        assignment_id: Uuid,
        student_id: Uuid,
        content: Option<String>,
        file_url: Option<String>,
        submitted_at: DateTime<Utc>,
        status: String,
        score: Option<i32>,
        feedback: Option<String>,
        graded_at: Option<DateTime<Utc>>,
        graded_by: Option<Uuid>,
        attempts: Vec<SubmissionAttempt>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        version: i32,
    ) -> Self {
        Self {
            id,
            tenant_id,
            assignment_id,
            student_id,
            content,
            file_url,
            submitted_at,
            status,
            score,
            feedback,
            graded_at,
            graded_by,
            attempts,
            created_at,
            updated_at,
            domain_events: Vec::new(),
            version,
        }
    }
}
