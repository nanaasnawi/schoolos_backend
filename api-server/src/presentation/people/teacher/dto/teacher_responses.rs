use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct TeacherSummaryResponse {
    pub id: Uuid,
    pub nip: Option<String>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub subject: Option<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct TeacherDetailResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub nip: Option<String>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub subject: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
