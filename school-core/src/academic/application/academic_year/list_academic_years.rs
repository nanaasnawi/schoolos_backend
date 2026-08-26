use crate::academic::domain::academic_year::AcademicYear;
use crate::academic::infrastructure::repository_traits::AcademicYearRepository;
use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListAcademicYearsQuery {
    pub tenant_id: Uuid,
    pub pagination: Pagination,
}

pub struct ListAcademicYearsUseCase {
    #[allow(dead_code)]
    academic_year_repo: Arc<dyn AcademicYearRepository>,
}

impl ListAcademicYearsUseCase {
    pub fn new(academic_year_repo: Arc<dyn AcademicYearRepository>) -> Self {
        Self { academic_year_repo }
    }

    pub async fn execute(
        &self,
        query: ListAcademicYearsQuery,
    ) -> Result<Page<AcademicYear>, ApplicationError> {
        Ok(Page::empty(
            query.pagination.page,
            query.pagination.page_size,
        ))
    }
}
