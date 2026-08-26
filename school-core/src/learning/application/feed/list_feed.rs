use crate::common::error::ApplicationError;
use crate::common::models::page::Page;
use crate::learning::domain::classroom_feed::FeedItem;
use crate::learning::infrastructure::repository_traits::FeedRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListFeedQuery {
    pub class_id: Uuid,
    pub tenant_id: Uuid,
    pub page: u32,
    pub per_page: u32,
}

pub struct ListFeedUseCase {
    repo: Arc<dyn FeedRepository>,
}

impl ListFeedUseCase {
    pub fn new(repo: Arc<dyn FeedRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: ListFeedQuery) -> Result<Page<FeedItem>, ApplicationError> {
        let page_size = query.per_page.max(1) as u64;
        let page = query.page.max(1) as u64;
        let offset = ((page - 1) * page_size) as i64;
        let limit = page_size as i64;

        let total = self
            .repo
            .count_by_class(query.class_id, query.tenant_id)
            .await?;
        let items = self
            .repo
            .find_by_class(query.class_id, query.tenant_id, limit, offset)
            .await?;

        Ok(Page::new(items, total, page, page_size))
    }
}
