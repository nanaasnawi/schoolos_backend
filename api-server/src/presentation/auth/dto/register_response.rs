use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterResponse {
    pub message: String,
    pub user_id: String,
}
