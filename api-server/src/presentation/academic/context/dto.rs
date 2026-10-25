use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AcademicContextResponse {
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub semester: i16,
    pub semester_type: String,
    pub semester_label: String,
    pub is_even_semester: bool,
    pub can_graduate_or_promote: bool,
    pub report_date: Option<String>,
    pub start_date: String,
    pub end_date: String,

    // Atomic School SSOT Metadata
    pub school_id: Option<Uuid>,
    pub school_name: String,
    pub npsn: Option<String>,
    pub logo_url: Option<String>,
    pub headmaster_name: Option<String>,
    pub headmaster_nip: Option<String>,
    pub address: Option<String>,
    pub village: Option<String>,
    pub district: Option<String>,
    pub regency: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateAcademicContextRequest {
    pub academic_year_id: Option<Uuid>,
    pub semester: Option<i16>,
    pub report_date: Option<NaiveDate>,
}
