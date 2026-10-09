use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartSessionRequest {
    pub session_type: Option<String>,
    pub schedule_id: Option<Uuid>,
    pub lesson_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub session_date: Option<chrono::NaiveDate>,
    pub session_number: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub substitute_teacher_id: Option<Uuid>,
    pub notes: Option<String>,
}
