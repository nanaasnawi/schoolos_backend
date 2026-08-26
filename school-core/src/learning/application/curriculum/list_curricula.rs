use crate::common::error::ApplicationError;
use crate::learning::domain::curriculum::Curriculum;
use crate::learning::infrastructure::repository_traits::CurriculumRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListCurriculaQuery {
    pub tenant_id: Uuid,
}

pub struct ListCurriculaUseCase {
    curriculum_repo: Arc<dyn CurriculumRepository>,
}

impl ListCurriculaUseCase {
    pub fn new(curriculum_repo: Arc<dyn CurriculumRepository>) -> Self {
        Self { curriculum_repo }
    }

    pub async fn execute(
        &self,
        query: ListCurriculaQuery,
    ) -> Result<Vec<Curriculum>, ApplicationError> {
        Ok(self.curriculum_repo.find_by_tenant(query.tenant_id).await?)
    }
}
