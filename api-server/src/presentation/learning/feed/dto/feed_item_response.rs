use chrono::{DateTime, Utc};
use school_core::learning::domain::classroom_feed::FeedItem;
use serde::Serialize;
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct FeedItemResponse {
    pub id: Uuid,
    pub class_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub summary: String,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
}

impl From<FeedItem> for FeedItemResponse {
    fn from(f: FeedItem) -> Self {
        Self {
            id: f.id,
            class_id: f.class_id,
            actor_id: f.actor_id,
            actor_name: f.actor_name,
            action: f.action,
            target_type: f.target_type,
            target_id: f.target_id,
            summary: f.summary,
            metadata: f.metadata,
            created_at: f.created_at,
        }
    }
}
