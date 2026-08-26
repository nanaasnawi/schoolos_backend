use crate::common::error::ApplicationError;
use crate::learning::domain::achievement::Achievement;
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListAchievementsQuery {
    pub tenant_id: Uuid,
}

pub struct ListAchievementsUseCase {
    repo: Arc<dyn AchievementRepository>,
}

impl ListAchievementsUseCase {
    pub fn new(repo: Arc<dyn AchievementRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: ListAchievementsQuery,
    ) -> Result<Vec<Achievement>, ApplicationError> {
        Ok(self.repo.find_by_tenant(query.tenant_id).await?)
    }
}
