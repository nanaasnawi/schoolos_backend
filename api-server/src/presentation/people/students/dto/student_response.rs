use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StudentResponse {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub class_name: Option<String>,
}
