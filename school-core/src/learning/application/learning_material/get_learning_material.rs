use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetLearningMaterialQuery {
    pub tenant_id: Uuid,
    pub material_id: Uuid,
}

pub struct GetLearningMaterialUseCase {
    material_repo: Arc<dyn LearningMaterialRepository>,
}

impl GetLearningMaterialUseCase {
    pub fn new(material_repo: Arc<dyn LearningMaterialRepository>) -> Self {
        Self { material_repo }
    }

    pub async fn execute(
        &self,
        query: GetLearningMaterialQuery,
    ) -> Result<LearningMaterial, ApplicationError> {
        self.material_repo
            .find_by_id(query.material_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LearningMaterialNotFound,
                    format!("LearningMaterial {} not found", query.material_id),
                )
            })
    }
}
