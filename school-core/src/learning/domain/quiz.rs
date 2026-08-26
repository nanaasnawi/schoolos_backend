use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use crate::learning::domain::quiz_question::QuizQuestion;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{
    QuestionAddedEvent, QuestionRemovedEvent, QuizArchivedEvent, QuizClosedEvent, QuizCreatedEvent,
    QuizPublishedEvent,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuizStatus {
    Draft,
    Published,
    Closed,
    Archived,
}

impl QuizStatus {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Quiz {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub duration_minutes: i32,
    pub passing_score: i32,
    pub max_score: i32,
    pub max_attempts: i32,
    pub shuffle_questions: bool,
    pub shuffle_choices: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub questions_count: i32,
    pub questions: Vec<QuizQuestion>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Quiz {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            lesson_id: self.lesson_id,
            title: self.title.clone(),
            description: self.description.clone(),
            status: self.status.clone(),
            duration_minutes: self.duration_minutes,
            passing_score: self.passing_score,
            max_score: self.max_score,
            max_attempts: self.max_attempts,
            shuffle_questions: self.shuffle_questions,
            shuffle_choices: self.shuffle_choices,
            start_at: self.start_at,
            end_at: self.end_at,
            questions_count: self.questions_count,
            questions: self.questions.clone(),
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

impl AggregateRoot for Quiz {
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

impl Quiz {
    pub fn new(
        tenant_id: Uuid,
        lesson_id: Uuid,
        title: String,
        description: Option<String>,
        duration_minutes: i32,
        passing_score: i32,
        max_attempts: i32,
        shuffle_questions: bool,
        shuffle_choices: bool,
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        if tenant_id.is_nil() {
            return Err(DomainError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if lesson_id.is_nil() {
            return Err(DomainError::Validation(
                "lesson_id must not be nil".to_string(),
            ));
        }
        if title.trim().is_empty() {
            return Err(DomainError::Validation(
                "title must not be empty".to_string(),
            ));
        }
        if duration_minutes <= 0 {
            return Err(DomainError::Validation(
                "duration_minutes must be > 0".to_string(),
            ));
        }
        if !(0..=100).contains(&passing_score) {
            return Err(DomainError::Validation(
                "passing_score must be between 0 and 100".to_string(),
            ));
        }
        if max_attempts <= 0 {
            return Err(DomainError::Validation(
                "max_attempts must be >= 1".to_string(),
            ));
        }
        if let (Some(start), Some(end)) = (start_at, end_at) {
            if start >= end {
                return Err(DomainError::Validation(
                    "start_at must be before end_at".to_string(),
                ));
            }
        }

        let now = clock.now();
        let id = Uuid::now_v7();

        let mut quiz = Self {
            id,
            tenant_id,
            lesson_id,
            title: title.clone(),
            description,
            status: "draft".to_string(),
            duration_minutes,
            passing_score,
            max_score: 0,
            max_attempts,
            shuffle_questions,
            shuffle_choices,
            start_at,
            end_at,
            questions_count: 0,
            questions: Vec::new(),
            is_active: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        quiz.raise_event(QuizCreatedEvent {
            metadata: EventMetadata::new(
                "QuizCreated".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            quiz_id: id,
            lesson_id,
            title,
        });

        Ok(quiz)
    }

    /// Domain Invariant: Add Question
    pub fn add_question(
        &mut self,
        question: QuizQuestion,
        clock: &dyn Clock,
    ) -> Result<(), DomainError> {
        if self.status == "archived" {
            return Err(DomainError::Validation(
                "Cannot modify an archived quiz".to_string(),
            ));
        }

        self.max_score += question.points;
        self.questions.push(question.clone());
        self.questions_count = self.questions.len() as i32;
        self.updated_at = clock.now();

        self.raise_event(QuestionAddedEvent {
            metadata: EventMetadata::new(
                "QuestionAdded".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            quiz_id: self.id,
            question_id: question.id,
        });

        Ok(())
    }

    /// Domain Invariant: Remove Question
    pub fn remove_question(
        &mut self,
        question_id: Uuid,
        clock: &dyn Clock,
    ) -> Result<(), DomainError> {
        if self.status == "archived" {
            return Err(DomainError::Validation(
                "Cannot modify an archived quiz".to_string(),
            ));
        }

        if let Some(pos) = self.questions.iter().position(|q| q.id == question_id) {
            let removed = self.questions.remove(pos);
            self.max_score -= removed.points;
            self.questions_count = self.questions.len() as i32;
            self.updated_at = clock.now();

            self.raise_event(QuestionRemovedEvent {
                metadata: EventMetadata::new(
                    "QuestionRemoved".to_string(),
                    self.tenant_id,
                    self.id.to_string(),
                    None,
                    None,
                    None,
                    (self.version + 1) as u32,
                    clock,
                ),
                quiz_id: self.id,
                question_id,
            });
        }

        Ok(())
    }

    /// Domain Invariant: Publish Quiz
    /// Rules:
    /// - Associated Lesson status MUST be 'published'
    /// - Quiz MUST have at least 1 question
    pub fn publish(&mut self, lesson_status: &str, clock: &dyn Clock) -> Result<(), DomainError> {
        if lesson_status != "published" {
            return Err(DomainError::Validation(format!(
                "Cannot publish quiz when associated lesson status is '{}'",
                lesson_status
            )));
        }

        if self.questions.is_empty() && self.questions_count == 0 {
            return Err(DomainError::Validation(
                "Cannot publish quiz without any questions".to_string(),
            ));
        }

        self.status = "published".to_string();
        self.updated_at = clock.now();

        self.raise_event(QuizPublishedEvent {
            metadata: EventMetadata::new(
                "QuizPublished".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            quiz_id: self.id,
            lesson_id: self.lesson_id,
        });

        Ok(())
    }

    /// Domain Invariant: Close Quiz
    pub fn close(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "closed".to_string();
        self.updated_at = clock.now();

        self.raise_event(QuizClosedEvent {
            metadata: EventMetadata::new(
                "QuizClosed".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            quiz_id: self.id,
        });

        Ok(())
    }

    /// Domain Invariant: Archive Quiz
    pub fn archive(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "archived".to_string();
        self.updated_at = clock.now();

        self.raise_event(QuizArchivedEvent {
            metadata: EventMetadata::new(
                "QuizArchived".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            quiz_id: self.id,
        });

        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
