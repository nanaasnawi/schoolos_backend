use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct StudentSummaryResponse {
    pub id: Uuid,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub religion: Option<String>,
    pub nipd: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub class_name: Option<String>,
    pub grade: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct StudentProfileResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub guardian_id: Option<Uuid>,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub religion: Option<String>,
    pub nipd: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub class_name: Option<String>,
    pub grade: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub guardian: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub current_class: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub current_enrollment: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub academic_year: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub attendance_summary: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub latest_assessment_summary: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
