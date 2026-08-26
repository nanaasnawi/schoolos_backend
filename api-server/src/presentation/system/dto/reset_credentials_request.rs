use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResetCredentialsRequest {
    pub current_email: String,
    pub new_email: Option<String>,
    pub new_password: Option<String>,
}
