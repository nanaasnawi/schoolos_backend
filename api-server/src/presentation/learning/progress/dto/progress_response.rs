use chrono::{DateTime, Utc};
use school_core::learning::domain::student_progress::StudentProgress;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProgressResponse {
    pub id: Uuid,
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
}

impl From<StudentProgress> for ProgressResponse {
    fn from(p: StudentProgress) -> Self {
        Self {
            id: p.id,
            student_id: p.student_id,
            class_id: p.class_id,
            subject_id: p.subject_id,
            overall_progress: p.overall_progress,
            lesson_completed: p.lesson_completed,
            lesson_total: p.lesson_total,
            assignment_completed: p.assignment_completed,
            assignment_total: p.assignment_total,
            quiz_completed: p.quiz_completed,
            quiz_total: p.quiz_total,
            session_attended: p.session_attended,
            session_total: p.session_total,
            calculated_at: p.calculated_at,
        }
    }
}
