use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteLearningMaterialCommand {
    pub tenant_id: Uuid,
    pub material_id: Uuid,
    pub deleted_by: Uuid,
}

pub struct DeleteLearningMaterialUseCase {
    material_repo: Arc<dyn LearningMaterialRepository>,
}

impl DeleteLearningMaterialUseCase {
    pub fn new(material_repo: Arc<dyn LearningMaterialRepository>) -> Self {
        Self { material_repo }
    }

    pub async fn execute(
        &self,
        command: DeleteLearningMaterialCommand,
    ) -> Result<(), ApplicationError> {
        let material = self
            .material_repo
            .find_by_id(command.material_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LearningMaterialNotFound,
                    format!("LearningMaterial {} not found", command.material_id),
                )
            })?;

        self.material_repo
            .delete(material.id, command.deleted_by)
            .await?;

        Ok(())
    }
}
