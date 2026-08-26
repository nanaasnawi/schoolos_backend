use crate::academic::domain::grade_level::GradeLevel;
use crate::academic::infrastructure::repository_traits::GradeLevelRepository;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetGradeLevelQuery {
    pub tenant_id: Uuid,
    pub grade_level_id: Uuid,
}

pub struct GetGradeLevelUseCase {
    repo: Arc<dyn GradeLevelRepository>,
}

impl GetGradeLevelUseCase {
    pub fn new(repo: Arc<dyn GradeLevelRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: GetGradeLevelQuery) -> Result<GradeLevel, ApplicationError> {
        let grade_level = self
            .repo
            .find_by_id(query.grade_level_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::GradeLevelNotFound,
                    format!("Grade level {} not found", query.grade_level_id),
                )
            })?;

        if grade_level.tenant_id != query.tenant_id {
            return Err(ApplicationError::NotFound(
                ErrorCode::GradeLevelNotFound,
                format!("Grade level {} not found for tenant", query.grade_level_id),
            ));
        }

        Ok(grade_level)
    }
}
