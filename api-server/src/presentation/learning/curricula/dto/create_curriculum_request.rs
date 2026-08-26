use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCurriculumRequest {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}
