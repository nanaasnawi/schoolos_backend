use crate::academic::domain::grade_level::GradeLevel;
use crate::academic::infrastructure::repository_traits::GradeLevelRepository;
use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListGradeLevelsQuery {
    pub tenant_id: Uuid,
    pub pagination: Pagination,
}

pub struct ListGradeLevelsUseCase {
    repo: Arc<dyn GradeLevelRepository>,
}

impl ListGradeLevelsUseCase {
    pub fn new(repo: Arc<dyn GradeLevelRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: ListGradeLevelsQuery,
    ) -> Result<Page<GradeLevel>, ApplicationError> {
        self.repo
            .list(
                query.tenant_id,
                query.pagination.page,
                query.pagination.page_size,
            )
            .await
            .map_err(Into::into)
    }
}
