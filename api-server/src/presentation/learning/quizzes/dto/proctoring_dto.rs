use chrono::{DateTime, Utc};
use school_core::learning::domain::proctoring::ProctorScheduleConflict;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::quiz_response::QuizResponse;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorSessionOverviewDto {
    pub id: Uuid,
    pub title: String,
    pub subject_name: Option<String>,
    pub class_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub teacher_id: Option<Uuid>,
    pub teacher_name: Option<String>,
    pub exam_mode: String,
    pub assessment_category: String,
    pub status: String,
    pub exam_phase: String,
    pub duration_minutes: i32,
    pub questions_count: i32,
    pub passing_score: i32,
    pub exam_token: Option<String>,
    pub requires_token: bool,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub release_at: Option<DateTime<Utc>>,
    pub seconds_until_start: Option<i64>,
    pub seconds_remaining: Option<i64>,
    pub total_enrolled: i64,
    pub started_count: i64,
    pub in_progress_count: i64,
    pub need_grading_count: i64,
    pub completed_count: i64,
    pub not_started_count: i64,
    pub total_violations: i64,
    pub flagged_students_count: i64,
    pub has_conflict: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorScheduleSummaryDto {
    pub total_sessions: usize,
    pub live_count: usize,
    pub upcoming_count: usize,
    pub ended_count: usize,
    pub draft_count: usize,
    pub conflict_count: usize,
    pub flagged_students_total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorScheduleConflictDto {
    pub conflict_type: String,
    pub primary_quiz_id: Uuid,
    pub primary_title: String,
    pub conflicting_quiz_id: Uuid,
    pub conflicting_title: String,
    pub scope_label: String,
    pub overlap_start: DateTime<Utc>,
    pub overlap_end: DateTime<Utc>,
}

impl From<ProctorScheduleConflict> for ProctorScheduleConflictDto {
    fn from(c: ProctorScheduleConflict) -> Self {
        Self {
            conflict_type: c.conflict_type,
            primary_quiz_id: c.primary_quiz_id,
            primary_title: c.primary_title,
            conflicting_quiz_id: c.conflicting_quiz_id,
            conflicting_title: c.conflicting_title,
            scope_label: c.scope_label,
            overlap_start: c.overlap_start,
            overlap_end: c.overlap_end,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorScheduleDashboardResponse {
    pub server_now: DateTime<Utc>,
    pub summary: ProctorScheduleSummaryDto,
    pub conflicts: Vec<ProctorScheduleConflictDto>,
    pub sessions: Vec<ProctorSessionOverviewDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorParticipantRowDto {
    pub student_id: Uuid,
    pub user_id: Option<Uuid>,
    pub student_name: String,
    pub student_nisn: String,
    pub class_name: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub raw_attempt_status: Option<String>,
    pub proctor_status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub elapsed_seconds: Option<i64>,
    pub answered_count: i64,
    pub total_questions: i32,
    pub score: Option<i32>,
    pub total_points: i32,
    pub percentage: Option<i32>,
    pub passed: Option<bool>,
    pub leave_violations_count: i32,
    pub last_violation_at: Option<DateTime<Utc>>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub is_online: bool,
    pub last_device_platform: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorEventDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub attempt_id: Uuid,
    pub event_type: String,
    pub detail: Option<String>,
    pub device_platform: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorMonitorSummaryDto {
    pub total_enrolled: usize,
    pub not_started_count: usize,
    pub in_progress_count: usize,
    pub need_grading_count: usize,
    pub completed_count: usize,
    pub flagged_count: usize,
    pub total_violations: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProctorMonitorResponse {
    pub server_now: DateTime<Utc>,
    pub quiz: QuizResponse,
    pub summary: ProctorMonitorSummaryDto,
    pub participants: Vec<ProctorParticipantRowDto>,
    pub recent_events: Vec<ProctorEventDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RotateTokenRequest {
    #[serde(default)]
    pub custom_token: Option<String>,
    #[serde(default)]
    pub expires_in_minutes: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtendTimeRequest {
    pub extra_minutes: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecordProctorEventRequest {
    pub event_type: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub device_platform: Option<String>,
}
