use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use crate::learning::domain::attempt_answer::AttemptAnswer;
use crate::learning::domain::quiz::Quiz;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{QuizAttemptStartedEvent, QuizAttemptSubmittedEvent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttemptStatus {
    Created,
    Started,
    InProgress,
    Submitted,
    AutoGraded,
    Reviewed,
}

impl AttemptStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Started => "started",
            Self::InProgress => "in_progress",
            Self::Submitted => "submitted",
            Self::AutoGraded => "auto_graded",
            Self::Reviewed => "reviewed",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuizAttempt {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
    pub attempt_number: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub score: Option<i32>,
    pub total_points: i32,
    pub passed: bool,
    pub status: String,
    pub shuffle_seed: i64,
    pub answers: Vec<AttemptAnswer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for QuizAttempt {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            quiz_id: self.quiz_id,
            student_id: self.student_id,
            attempt_number: self.attempt_number,
            started_at: self.started_at,
            completed_at: self.completed_at,
            score: self.score,
            total_points: self.total_points,
            passed: self.passed,
            status: self.status.clone(),
            shuffle_seed: self.shuffle_seed,
            answers: self.answers.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for QuizAttempt {
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

impl QuizAttempt {
    pub fn start_new(
        tenant_id: Uuid,
        quiz: &Quiz,
        student_id: Uuid,
        previous_attempts_count: i32,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        if quiz.status != "published" {
            return Err(DomainError::Validation(format!(
                "Cannot start attempt for a quiz with status '{}'",
                quiz.status
            )));
        }

        if previous_attempts_count >= quiz.max_attempts {
            return Err(DomainError::Validation(format!(
                "Maximum attempt limit ({}) reached for quiz",
                quiz.max_attempts
            )));
        }

        let now = clock.now();
        if let Some(start_at) = quiz.start_at {
            if now < start_at {
                return Err(DomainError::Validation(
                    "Quiz has not opened yet".to_string(),
                ));
            }
        }
        if let Some(end_at) = quiz.end_at {
            if now > end_at {
                return Err(DomainError::Validation(
                    "Quiz window has expired".to_string(),
                ));
            }
        }

        let id = Uuid::now_v7();
        let attempt_number = previous_attempts_count + 1;
        let shuffle_seed = now.timestamp_millis();

        let mut attempt = Self {
            id,
            tenant_id,
            quiz_id: quiz.id,
            student_id,
            attempt_number,
            started_at: now,
            completed_at: None,
            score: None,
            total_points: quiz.max_score,
            passed: false,
            status: "in_progress".to_string(),
            shuffle_seed,
            answers: Vec::new(),
            created_at: now,
            updated_at: now,
            domain_events: Vec::new(),
            version: 1,
        };

        attempt.raise_event(QuizAttemptStartedEvent {
            metadata: EventMetadata::new(
                "QuizAttemptStarted".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            attempt_id: id,
            quiz_id: quiz.id,
            student_id,
        });

        Ok(attempt)
    }

    pub fn add_answer(&mut self, answer: AttemptAnswer) {
        self.answers.push(answer);
    }

    /// Auto Grade Domain Method
    /// Automatically calculates earned points, score percentage, and pass/fail state
    pub fn auto_grade(
        &mut self,
        quiz: &Quiz,
        clock: &dyn Clock,
    ) -> Result<(i32, bool), DomainError> {
        let mut earned_score = 0;

        for answer in &mut self.answers {
            if let Some(question) = quiz.questions.iter().find(|q| q.id == answer.question_id) {
                if let Some(chosen_id) = answer.chosen_choice_id {
                    let is_correct = question
                        .choices
                        .iter()
                        .any(|c| c.id == chosen_id && c.is_correct);

                    answer.is_correct = Some(is_correct);
                    answer.points_earned = if is_correct { question.points } else { 0 };
                    earned_score += answer.points_earned;
                }
            }
        }

        let percentage = if quiz.max_score > 0 {
            (earned_score * 100) / quiz.max_score
        } else {
            0
        };

        let passed = percentage >= quiz.passing_score;
        let now = clock.now();

        self.score = Some(earned_score);
        self.passed = passed;
        self.status = "auto_graded".to_string();
        self.completed_at = Some(now);
        self.updated_at = now;

        self.raise_event(QuizAttemptSubmittedEvent {
            metadata: EventMetadata::new(
                "QuizAttemptSubmitted".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            attempt_id: self.id,
            quiz_id: quiz.id,
            student_id: self.student_id,
            score: earned_score,
            passed,
        });

        Ok((earned_score, passed))
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
