use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "admin@schoolos.id")]
    pub email: Option<String>,

    #[schema(example = "0092960256")]
    pub username: Option<String>,

    #[schema(example = "secretpassword")]
    pub password: String,
}
