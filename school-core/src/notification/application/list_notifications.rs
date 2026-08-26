use crate::common::error::ApplicationError;
use crate::common::models::page::Page;
use crate::notification::domain::notification::Notification;
use crate::notification::infrastructure::repository_traits::NotificationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListNotificationsQuery {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub page: u32,
    pub per_page: u32,
}

pub struct ListNotificationsUseCase {
    repo: Arc<dyn NotificationRepository>,
}

impl ListNotificationsUseCase {
    pub fn new(repo: Arc<dyn NotificationRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: ListNotificationsQuery,
    ) -> Result<Page<Notification>, ApplicationError> {
        let page_size = query.per_page.max(1) as u64;
        let page = query.page.max(1) as u64;
        let offset = ((page - 1) * page_size) as i64;
        let limit = page_size as i64;

        let total = self
            .repo
            .count_by_user(query.user_id, query.tenant_id)
            .await?;
        let items = self
            .repo
            .find_by_user(query.user_id, query.tenant_id, limit, offset)
            .await?;

        Ok(Page::new(items, total, page, page_size))
    }
}
