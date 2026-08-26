use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertPreferenceRequest {
    pub notification_type: String,
    pub channels: Vec<String>,
    pub is_enabled: bool,
}
