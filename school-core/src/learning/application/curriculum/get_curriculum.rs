use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::curriculum::Curriculum;
use crate::learning::infrastructure::repository_traits::CurriculumRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetCurriculumQuery {
    pub tenant_id: Uuid,
    pub curriculum_id: Uuid,
}

pub struct GetCurriculumUseCase {
    curriculum_repo: Arc<dyn CurriculumRepository>,
}

impl GetCurriculumUseCase {
    pub fn new(curriculum_repo: Arc<dyn CurriculumRepository>) -> Self {
        Self { curriculum_repo }
    }

    pub async fn execute(&self, query: GetCurriculumQuery) -> Result<Curriculum, ApplicationError> {
        self.curriculum_repo
            .find_by_id(query.curriculum_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::CurriculumNotFound,
                    format!("Curriculum {} not found", query.curriculum_id),
                )
            })
    }
}
