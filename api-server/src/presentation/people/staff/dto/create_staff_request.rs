use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateStaffRequest {
    pub full_name: String,
    pub job_title: String,
}
