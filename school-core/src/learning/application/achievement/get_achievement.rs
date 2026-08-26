use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::achievement::Achievement;
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetAchievementQuery {
    pub achievement_id: Uuid,
}

pub struct GetAchievementUseCase {
    repo: Arc<dyn AchievementRepository>,
}

impl GetAchievementUseCase {
    pub fn new(repo: Arc<dyn AchievementRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetAchievementQuery,
    ) -> Result<Achievement, ApplicationError> {
        self.repo
            .find_by_id(query.achievement_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AchievementNotFound,
                    format!("Achievement {} not found", query.achievement_id),
                )
            })
    }
}
