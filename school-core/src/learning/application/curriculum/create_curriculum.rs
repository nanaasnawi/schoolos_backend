use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::curriculum::Curriculum;
use crate::learning::infrastructure::repository_traits::CurriculumRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateCurriculumCommand {
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

pub struct CreateCurriculumUseCase {
    curriculum_repo: Arc<dyn CurriculumRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateCurriculumUseCase {
    pub fn new(curriculum_repo: Arc<dyn CurriculumRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            curriculum_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateCurriculumCommand,
    ) -> Result<Curriculum, ApplicationError> {
        let curriculum = Curriculum::new(
            command.tenant_id,
            command.code,
            command.name,
            command.description,
            &*self.clock,
        );

        self.curriculum_repo.create(&curriculum).await?;

        Ok(curriculum)
    }
}
