use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RegisterRequest {
    #[schema(example = "admin@schoolos.id")]
    pub email: String,
    #[schema(example = "admin123")]
    pub password: String,
    #[schema(example = "System Administrator")]
    pub full_name: String,
    #[schema(example = "Administrator")]
    pub role: Option<String>,
}
