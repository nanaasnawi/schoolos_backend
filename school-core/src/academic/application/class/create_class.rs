use crate::academic::domain::class::Class;
use crate::academic::infrastructure::repository_traits::{AcademicYearRepository, ClassRepository};
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateClassCommand {
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
}

pub struct CreateClassUseCase {
    class_repo: Arc<dyn ClassRepository>,
    academic_year_repo: Arc<dyn AcademicYearRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateClassUseCase {
    pub fn new(
        class_repo: Arc<dyn ClassRepository>,
        academic_year_repo: Arc<dyn AcademicYearRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            class_repo,
            academic_year_repo,
            clock,
        }
    }

    pub async fn execute(&self, command: CreateClassCommand) -> Result<Class, ApplicationError> {
        // Validate academic year exists
        let _ = self
            .academic_year_repo
            .find_by_id(command.academic_year_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    crate::common::error_code::ErrorCode::AcademicYearNotFound,
                    format!("Academic Year not found: {}", command.academic_year_id),
                )
            })?;

        let class = Class::new(
            command.tenant_id,
            command.academic_year_id,
            command.grade_level_id,
            command.name,
            30, // Default capacity
            &*self.clock,
        );

        self.class_repo.create(&class).await?;

        Ok(class)
    }
}
