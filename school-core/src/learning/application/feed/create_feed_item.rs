use crate::common::error::ApplicationError;
use crate::learning::domain::classroom_feed::FeedItem;
use crate::learning::infrastructure::repository_traits::FeedRepository;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateFeedItemCommand {
    pub tenant_id: Uuid,
    pub class_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub summary: String,
    pub metadata: JsonValue,
}

pub struct CreateFeedItemUseCase {
    repo: Arc<dyn FeedRepository>,
}

impl CreateFeedItemUseCase {
    pub fn new(repo: Arc<dyn FeedRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        command: CreateFeedItemCommand,
    ) -> Result<FeedItem, ApplicationError> {
        let item = FeedItem::new(
            command.tenant_id,
            command.class_id,
            command.actor_id,
            command.actor_name,
            command.action,
            command.target_type,
            command.target_id,
            command.summary,
            command.metadata,
        );

        self.repo.create(&item).await?;
        Ok(item)
    }
}
