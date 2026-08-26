use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    #[schema(example = "Bearer")]
    pub token_type: String,

    #[schema(example = 86400)]
    pub expires_in: usize,
}
