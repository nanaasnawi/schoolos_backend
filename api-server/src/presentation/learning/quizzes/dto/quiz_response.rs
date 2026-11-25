use chrono::{DateTime, Utc};
use school_core::learning::domain::proctoring::{evaluate_quiz_schedule, QuizScheduleInput};
use school_core::learning::domain::quiz::Quiz;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct QuizResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub duration_minutes: i32,
    pub time_limit_minutes: i32,
    pub passing_score: i32,
    pub max_score: i32,
    pub max_attempts: i32,
    pub shuffle_questions: bool,
    pub shuffle_choices: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub status: String,
    pub questions_count: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_at: Option<DateTime<Utc>>,
    #[serde(default = "default_exam_mode")]
    pub exam_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessment_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exam_token: Option<String>,
    #[serde(default)]
    pub requires_token: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_expires_at: Option<DateTime<Utc>>,
    #[serde(default = "default_max_token_attempts")]
    pub max_token_attempts: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_attempt_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_attempts_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_has_completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_last_score: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_last_attempt_id: Option<Uuid>,
    #[serde(default = "default_exam_phase")]
    pub exam_phase: String,
    #[serde(default)]
    pub is_open_for_student: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds_until_start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds_remaining: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_students: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_score: Option<f64>,
}

fn default_exam_mode() -> Option<String> {
    Some("HOMEWORK_QUIZ".to_string())
}

fn default_max_token_attempts() -> Option<i32> {
    Some(5)
}

fn default_exam_phase() -> String {
    "DRAFT".to_string()
}

impl QuizResponse {
    /// Populates canonical server-computed schedule phase and student availability flags
    /// from the domain state evaluator (ADR-0011, ADR-0013).
    pub fn apply_schedule_evaluation(&mut self, now: DateTime<Utc>) {
        let eval = evaluate_quiz_schedule(QuizScheduleInput {
            status: &self.status,
            is_active: self.is_active,
            questions_count: self.questions_count,
            start_at: self.start_at,
            end_at: self.end_at,
            release_at: self.release_at,
            student_attempt_status: self.student_attempt_status.as_deref(),
            student_has_completed: self.student_has_completed.unwrap_or(false),
            now,
        });
        self.exam_phase = eval.phase.as_str().to_string();
        self.is_open_for_student = eval.is_open_for_student;
        self.seconds_until_start = eval.seconds_until_start;
        self.seconds_remaining = eval.seconds_remaining;

        if eval.phase == school_core::learning::domain::proctoring::ExamPhase::Ended {
            if !self.status.eq_ignore_ascii_case("draft")
                && !self.status.eq_ignore_ascii_case("archived")
            {
                self.status = "closed".to_string();
            }
            self.is_active = false;
        }
    }
}

impl From<Quiz> for QuizResponse {
    fn from(q: Quiz) -> Self {
        let mut resp = Self {
            id: q.id,
            tenant_id: q.tenant_id,
            lesson_id: q.lesson_id,
            title: q.title,
            description: q.description,
            duration_minutes: q.duration_minutes,
            time_limit_minutes: q.duration_minutes,
            passing_score: q.passing_score,
            max_score: q.max_score,
            max_attempts: q.max_attempts,
            shuffle_questions: q.shuffle_questions,
            shuffle_choices: q.shuffle_choices,
            start_at: q.start_at,
            end_at: q.end_at,
            status: q.status,
            questions_count: q.questions_count,
            is_active: q.is_active,
            created_at: q.created_at,
            updated_at: q.updated_at,
            class_id: None,
            class_name: None,
            subject_name: None,
            teacher_id: None,
            teacher_name: None,
            session_id: None,
            release_at: None,
            exam_mode: Some("HOMEWORK_QUIZ".to_string()),
            assessment_category: Some("FORMATIVE".to_string()),
            exam_token: None,
            requires_token: false,
            token_expires_at: None,
            max_token_attempts: Some(5),
            student_attempt_status: None,
            student_attempts_count: None,
            student_has_completed: None,
            student_last_score: None,
            student_last_attempt_id: None,
            exam_phase: default_exam_phase(),
            is_open_for_student: false,
            seconds_until_start: None,
            seconds_remaining: None,
            total_students: None,
            participants_count: None,
            avg_score: None,
        };
        resp.apply_schedule_evaluation(Utc::now());
        resp
    }
}
