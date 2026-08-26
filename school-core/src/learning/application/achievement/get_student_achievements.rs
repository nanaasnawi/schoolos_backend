use crate::common::error::ApplicationError;
use crate::learning::domain::achievement::StudentAchievement;
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetStudentAchievementsQuery {
    pub student_id: Uuid,
    pub tenant_id: Uuid,
}

pub struct GetStudentAchievementsUseCase {
    repo: Arc<dyn AchievementRepository>,
}

impl GetStudentAchievementsUseCase {
    pub fn new(repo: Arc<dyn AchievementRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetStudentAchievementsQuery,
    ) -> Result<Vec<StudentAchievement>, ApplicationError> {
        Ok(self
            .repo
            .find_student_achievements(query.student_id, query.tenant_id)
            .await?)
    }
}
