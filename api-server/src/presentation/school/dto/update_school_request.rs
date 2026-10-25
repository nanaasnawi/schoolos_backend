use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSchoolRequest {
    pub name: Option<String>,
    pub npsn: Option<String>,
    pub logo_url: Option<String>,
    pub address: Option<String>,
    pub village: Option<String>,
    pub district: Option<String>,
    pub regency: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub headmaster_name: Option<String>,
    pub headmaster_nip: Option<String>,
    pub status: Option<String>,
    pub dapodik_url: Option<String>,
    pub dapodik_token: Option<String>,
    pub accreditation: Option<String>,
}
