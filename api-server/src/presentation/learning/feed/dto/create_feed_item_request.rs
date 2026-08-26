use serde::Deserialize;
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFeedItemRequest {
    pub class_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub summary: String,
    pub metadata: Option<JsonValue>,
}
