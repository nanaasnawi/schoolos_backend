use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateGuardianRequest {
    pub full_name: Option<String>,
    pub phone_number: Option<String>,
}
