use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSubjectRequest {
    pub code: String,
    pub name: String,
}
