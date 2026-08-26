use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateLearningMaterialCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub material_type: String,
    pub title: String,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub order_index: i32,
    pub visibility: String,
}

pub struct CreateLearningMaterialUseCase {
    material_repo: Arc<dyn LearningMaterialRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateLearningMaterialUseCase {
    pub fn new(material_repo: Arc<dyn LearningMaterialRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            material_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateLearningMaterialCommand,
    ) -> Result<LearningMaterial, ApplicationError> {
        let mut material = LearningMaterial::new(
            command.tenant_id,
            command.lesson_id,
            command.material_type,
            command.title,
            command.description,
            command.storage_key,
            command.external_url,
            command.order_index,
            command.visibility,
            &*self.clock,
        );

        self.material_repo.create(&material).await?;

        // Drain domain events
        let _events = material.take_events();

        Ok(material)
    }
}
