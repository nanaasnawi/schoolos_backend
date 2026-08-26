use crate::academic::domain::grade_level::GradeLevel;
use crate::academic::infrastructure::repository_traits::GradeLevelRepository;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateGradeLevelCommand {
    pub tenant_id: Uuid,
    pub level: i32,
    pub name: String,
}

pub struct CreateGradeLevelUseCase {
    repo: Arc<dyn GradeLevelRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateGradeLevelUseCase {
    pub fn new(repo: Arc<dyn GradeLevelRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn execute(
        &self,
        command: CreateGradeLevelCommand,
    ) -> Result<GradeLevel, ApplicationError> {
        let grade_level =
            GradeLevel::new(command.tenant_id, command.level, command.name, &*self.clock);

        self.repo.create(&grade_level).await?;

        Ok(grade_level)
    }
}
