use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateGuardianRequest {
    pub full_name: String,
    pub phone_number: Option<String>,
}
