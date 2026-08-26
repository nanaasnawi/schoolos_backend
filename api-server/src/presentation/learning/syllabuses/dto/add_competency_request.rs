use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddCompetencyRequest {
    pub code: String,
    pub competency_type: String,
    pub description: String,
    pub order_index: i32,
}
