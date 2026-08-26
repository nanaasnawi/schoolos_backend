use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ActivateMasterRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub role_name: Option<String>,
}
