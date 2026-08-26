use crate::common::error::ApplicationError;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListLearningMaterialsQuery {
    pub tenant_id: Uuid,
}

pub struct ListLearningMaterialsUseCase {
    material_repo: Arc<dyn LearningMaterialRepository>,
}

impl ListLearningMaterialsUseCase {
    pub fn new(material_repo: Arc<dyn LearningMaterialRepository>) -> Self {
        Self { material_repo }
    }

    pub async fn execute(
        &self,
        query: ListLearningMaterialsQuery,
    ) -> Result<Vec<LearningMaterial>, ApplicationError> {
        Ok(self.material_repo.find_by_tenant(query.tenant_id).await?)
    }
}
