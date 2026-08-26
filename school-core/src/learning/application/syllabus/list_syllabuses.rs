use crate::common::error::ApplicationError;
use crate::learning::domain::syllabus::Syllabus;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListSyllabusesQuery {
    pub tenant_id: Uuid,
}

pub struct ListSyllabusesUseCase {
    syllabus_repo: Arc<dyn SyllabusRepository>,
}

impl ListSyllabusesUseCase {
    pub fn new(syllabus_repo: Arc<dyn SyllabusRepository>) -> Self {
        Self { syllabus_repo }
    }

    pub async fn execute(
        &self,
        query: ListSyllabusesQuery,
    ) -> Result<Vec<Syllabus>, ApplicationError> {
        Ok(self.syllabus_repo.find_by_tenant(query.tenant_id).await?)
    }
}
