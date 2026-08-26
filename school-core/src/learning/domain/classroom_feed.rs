use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
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

impl FeedItem {
    pub fn new(
        tenant_id: Uuid,
        class_id: Uuid,
        actor_id: Uuid,
        actor_name: String,
        action: String,
        target_type: Option<String>,
        target_id: Option<Uuid>,
        summary: String,
        metadata: JsonValue,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            class_id,
            actor_id,
            actor_name,
            action,
            target_type,
            target_id,
            summary,
            metadata,
            created_at: Utc::now(),
        }
    }
}
