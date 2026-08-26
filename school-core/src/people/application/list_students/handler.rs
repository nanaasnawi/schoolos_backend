use super::query::ListStudentsQuery;
use crate::common::error::ApplicationError;
use crate::common::models::page::Page;
use crate::people::domain::read_models::StudentSummary;
use crate::people::infrastructure::repository_traits::StudentQueryRepository;
use std::sync::Arc;

pub struct ListStudentsUseCase {
    student_repo: Arc<dyn StudentQueryRepository>,
}

impl ListStudentsUseCase {
    pub fn new(student_repo: Arc<dyn StudentQueryRepository>) -> Self {
        Self { student_repo }
    }

    pub async fn execute(
        &self,
        query: ListStudentsQuery,
    ) -> Result<Page<StudentSummary>, ApplicationError> {
        let students = self.student_repo.search(query).await?;
        Ok(students)
    }
}
