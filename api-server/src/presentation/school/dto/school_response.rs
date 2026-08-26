use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SchoolResponse {
    #[schema(example = "00000000-0000-0000-0000-000000000001")]
    pub id: Uuid,

    #[schema(example = "PKBM AS-SALAFIYAH")]
    pub name: String,

    #[schema(example = "P2962010")]
    pub npsn: Option<String>,

    #[schema(example = "https://example.com/logo.png")]
    pub logo_url: Option<String>,

    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub dapodik_url: Option<String>,
    pub dapodik_token: Option<String>,
    pub accreditation: Option<String>,

    #[schema(example = "00000000-0000-0000-0000-000000000001")]
    pub tenant_id: Uuid,

    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight public school info — no auth required.
/// Returned by the /schools/info public endpoint for use on the mobile login screen.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SchoolPublicInfo {
    pub name: String,
    pub logo_url: Option<String>,
    pub npsn: Option<String>,
}
