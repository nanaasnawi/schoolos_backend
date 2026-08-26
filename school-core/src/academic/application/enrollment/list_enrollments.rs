use crate::academic::domain::enrollment::Enrollment;
use crate::academic::infrastructure::repository_traits::EnrollmentRepository;
use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListEnrollmentsQuery {
    pub tenant_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub pagination: Pagination,
}

pub struct ListEnrollmentsUseCase {
    #[allow(dead_code)]
    enrollment_repo: Arc<dyn EnrollmentRepository>,
}

impl ListEnrollmentsUseCase {
    pub fn new(enrollment_repo: Arc<dyn EnrollmentRepository>) -> Self {
        Self { enrollment_repo }
    }

    pub async fn execute(
        &self,
        query: ListEnrollmentsQuery,
    ) -> Result<Page<Enrollment>, ApplicationError> {
        Ok(Page::empty(
            query.pagination.page,
            query.pagination.page_size,
        ))
    }
}
