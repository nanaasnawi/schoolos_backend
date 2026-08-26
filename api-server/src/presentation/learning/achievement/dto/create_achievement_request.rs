use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAchievementRequest {
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub criteria_type: String,
    pub criteria_value: String,
}
