use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateTenantRequest {
    pub school_name: String,
    pub npsn: Option<String>,
    pub master_full_name: String,
    pub master_email: String,
    pub master_password: String,
    pub master_role: Option<String>,
}
