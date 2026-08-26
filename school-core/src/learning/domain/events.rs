use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::domain::event::{DomainEvent, EventMetadata};

// ─── Learning Material Events ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningMaterialCreatedEvent {
    pub metadata: EventMetadata,
    pub material_id: Uuid,
    pub title: String,
    pub material_type: String,
}

impl DomainEvent for LearningMaterialCreatedEvent {
    fn event_name(&self) -> &'static str {
        "LearningMaterialCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningMaterialUpdatedEvent {
    pub metadata: EventMetadata,
    pub material_id: Uuid,
    pub title: String,
}

impl DomainEvent for LearningMaterialUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "LearningMaterialUpdated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Lesson Aggregate Events ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonCreatedEvent {
    pub metadata: EventMetadata,
    pub lesson_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub title: String,
}

impl DomainEvent for LessonCreatedEvent {
    fn event_name(&self) -> &'static str {
        "LessonCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonUpdatedEvent {
    pub metadata: EventMetadata,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for LessonUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "LessonUpdated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonPublishedEvent {
    pub metadata: EventMetadata,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for LessonPublishedEvent {
    fn event_name(&self) -> &'static str {
        "LessonPublished"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonArchivedEvent {
    pub metadata: EventMetadata,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for LessonArchivedEvent {
    fn event_name(&self) -> &'static str {
        "LessonArchived"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonDeletedEvent {
    pub metadata: EventMetadata,
    pub lesson_id: Uuid,
    pub deleted_by: Uuid,
}

impl DomainEvent for LessonDeletedEvent {
    fn event_name(&self) -> &'static str {
        "LessonDeleted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Assignment Aggregate Events ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentCreatedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for AssignmentCreatedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentUpdatedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub title: String,
}

impl DomainEvent for AssignmentUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentUpdated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentPublishedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for AssignmentPublishedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentPublished"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentClosedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub title: String,
}

impl DomainEvent for AssignmentClosedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentClosed"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentArchivedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub title: String,
}

impl DomainEvent for AssignmentArchivedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentArchived"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentDeletedEvent {
    pub metadata: EventMetadata,
    pub assignment_id: Uuid,
    pub deleted_by: Uuid,
}

impl DomainEvent for AssignmentDeletedEvent {
    fn event_name(&self) -> &'static str {
        "AssignmentDeleted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentSubmitted {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub assignment_id: Uuid,
    pub student_id: Uuid,
    pub tenant_id: Uuid,
}

impl DomainEvent for AssignmentSubmitted {
    fn event_name(&self) -> &'static str {
        "AssignmentSubmitted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeReleased {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub assignment_id: Uuid,
    pub student_id: Uuid,
    pub score: i32,
    pub tenant_id: Uuid,
}

impl DomainEvent for GradeReleased {
    fn event_name(&self) -> &'static str {
        "GradeReleased"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Submission Aggregate Events ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionCreatedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub assignment_id: Uuid,
    pub student_id: Uuid,
}

impl DomainEvent for SubmissionCreatedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionAttemptAddedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub attempt_number: i32,
    pub is_late: bool,
}

impl DomainEvent for SubmissionAttemptAddedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionAttemptAdded"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionGradingStartedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
}

impl DomainEvent for SubmissionGradingStartedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionGradingStarted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionGradedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub score: i32,
    pub graded_by: Uuid,
}

impl DomainEvent for SubmissionGradedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionGraded"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionReturnedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
}

impl DomainEvent for SubmissionReturnedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionReturned"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionDeletedEvent {
    pub metadata: EventMetadata,
    pub submission_id: Uuid,
    pub deleted_by: Uuid,
}

impl DomainEvent for SubmissionDeletedEvent {
    fn event_name(&self) -> &'static str {
        "SubmissionDeleted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Quiz Aggregate Events ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizCreatedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
}

impl DomainEvent for QuizCreatedEvent {
    fn event_name(&self) -> &'static str {
        "QuizCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionAddedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
    pub question_id: Uuid,
}

impl DomainEvent for QuestionAddedEvent {
    fn event_name(&self) -> &'static str {
        "QuestionAdded"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionRemovedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
    pub question_id: Uuid,
}

impl DomainEvent for QuestionRemovedEvent {
    fn event_name(&self) -> &'static str {
        "QuestionRemoved"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizPublishedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
    pub lesson_id: Uuid,
}

impl DomainEvent for QuizPublishedEvent {
    fn event_name(&self) -> &'static str {
        "QuizPublished"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizClosedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
}

impl DomainEvent for QuizClosedEvent {
    fn event_name(&self) -> &'static str {
        "QuizClosed"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizArchivedEvent {
    pub metadata: EventMetadata,
    pub quiz_id: Uuid,
}

impl DomainEvent for QuizArchivedEvent {
    fn event_name(&self) -> &'static str {
        "QuizArchived"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAttemptStartedEvent {
    pub metadata: EventMetadata,
    pub attempt_id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
}

impl DomainEvent for QuizAttemptStartedEvent {
    fn event_name(&self) -> &'static str {
        "QuizAttemptStarted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAttemptSubmittedEvent {
    pub metadata: EventMetadata,
    pub attempt_id: Uuid,
    pub quiz_id: Uuid,
    pub student_id: Uuid,
    pub score: i32,
    pub passed: bool,
}

impl DomainEvent for QuizAttemptSubmittedEvent {
    fn event_name(&self) -> &'static str {
        "QuizAttemptSubmitted"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Assessment Aggregate Events ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentRuleCreatedEvent {
    pub metadata: EventMetadata,
    pub rule_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

impl DomainEvent for AssessmentRuleCreatedEvent {
    fn event_name(&self) -> &'static str {
        "AssessmentRuleCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentRuleActivatedEvent {
    pub metadata: EventMetadata,
    pub rule_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

impl DomainEvent for AssessmentRuleActivatedEvent {
    fn event_name(&self) -> &'static str {
        "AssessmentRuleActivated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeEntryRecordedEvent {
    pub metadata: EventMetadata,
    pub gradebook_id: Uuid,
    pub student_id: Uuid,
    pub score: f64,
}

impl DomainEvent for GradeEntryRecordedEvent {
    fn event_name(&self) -> &'static str {
        "GradeEntryRecorded"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeCalculatedEvent {
    pub metadata: EventMetadata,
    pub gradebook_id: Uuid,
    pub student_id: Uuid,
    pub final_score: f64,
    pub passed: bool,
}

impl DomainEvent for GradeCalculatedEvent {
    fn event_name(&self) -> &'static str {
        "GradeCalculated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalGradePublishedEvent {
    pub metadata: EventMetadata,
    pub gradebook_id: Uuid,
    pub student_id: Uuid,
}

impl DomainEvent for FinalGradePublishedEvent {
    fn event_name(&self) -> &'static str {
        "FinalGradePublished"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
