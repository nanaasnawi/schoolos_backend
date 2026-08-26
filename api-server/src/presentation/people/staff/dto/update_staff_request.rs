use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateStaffRequest {
    pub full_name: Option<String>,
    pub job_title: Option<String>,
}
