use chrono::{DateTime, Utc};
use school_core::learning::domain::assignment_submission::AssignmentSubmission;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmissionResponse {
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
}

impl From<AssignmentSubmission> for SubmissionResponse {
    fn from(s: AssignmentSubmission) -> Self {
        Self {
            id: s.id,
            tenant_id: s.tenant_id,
            assignment_id: s.assignment_id,
            student_id: s.student_id,
            content: s.content,
            file_url: s.file_url,
            submitted_at: s.submitted_at,
            status: s.status,
            score: s.score,
            feedback: s.feedback,
            graded_at: s.graded_at,
            graded_by: s.graded_by,
        }
    }
}
