use crate::academic::domain::class::Class;
use crate::academic::infrastructure::repository_traits::ClassRepository;
use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListClassesQuery {
    pub tenant_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub pagination: Pagination,
}

pub struct ListClassesUseCase {
    class_repo: Arc<dyn ClassRepository>,
}

impl ListClassesUseCase {
    pub fn new(class_repo: Arc<dyn ClassRepository>) -> Self {
        Self { class_repo }
    }

    pub async fn execute(&self, query: ListClassesQuery) -> Result<Page<Class>, ApplicationError> {
        self.class_repo.list(
            query.tenant_id,
            query.academic_year_id,
            query.pagination.page,
            query.pagination.page_size,
        ).await.map_err(ApplicationError::Infrastructure)
    }
}
