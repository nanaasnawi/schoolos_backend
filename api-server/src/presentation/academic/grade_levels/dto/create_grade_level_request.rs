use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGradeLevelRequest {
    pub level: i32,
    pub name: String,
}
