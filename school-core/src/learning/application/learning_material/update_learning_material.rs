use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateLearningMaterialCommand {
    pub tenant_id: Uuid,
    pub material_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub visibility: Option<String>,
}

pub struct UpdateLearningMaterialUseCase {
    material_repo: Arc<dyn LearningMaterialRepository>,
}

impl UpdateLearningMaterialUseCase {
    pub fn new(material_repo: Arc<dyn LearningMaterialRepository>) -> Self {
        Self { material_repo }
    }

    pub async fn execute(
        &self,
        command: UpdateLearningMaterialCommand,
    ) -> Result<LearningMaterial, ApplicationError> {
        let mut material = self
            .material_repo
            .find_by_id(command.material_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LearningMaterialNotFound,
                    format!("LearningMaterial {} not found", command.material_id),
                )
            })?;

        if let Some(title) = command.title {
            if !title.is_empty() {
                material.title = title;
            }
        }
        if let Some(desc) = command.description {
            material.description = Some(desc);
        }
        if let Some(key) = command.storage_key {
            material.storage_key = Some(key);
        }
        if let Some(url) = command.external_url {
            material.external_url = Some(url);
        }
        if let Some(vis) = command.visibility {
            material.visibility = vis;
        }

        self.material_repo.update(&material).await?;

        Ok(material)
    }
}
