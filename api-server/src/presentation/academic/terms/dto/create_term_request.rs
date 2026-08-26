use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTermRequest {
    pub academic_year_id: Uuid,
    pub name: String,
}
