use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct StaffSummaryResponse {
    pub id: Uuid,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub nip: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub job_title: String,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct StaffDetailResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub nip: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub job_title: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
