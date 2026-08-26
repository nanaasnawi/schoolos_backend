use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::ApplicationError;
use crate::common::error::DomainError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::achievement::{AchievementEarned, StudentAchievement};
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct AwardAchievementCommand {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub achievement_id: Uuid,
}

pub struct AwardAchievementUseCase {
    repo: Arc<dyn AchievementRepository>,
    event_bus: SharedEventBus,
}

impl AwardAchievementUseCase {
    pub fn new(repo: Arc<dyn AchievementRepository>, event_bus: SharedEventBus) -> Self {
        Self { repo, event_bus }
    }

    pub async fn execute(
        &self,
        command: AwardAchievementCommand,
    ) -> Result<StudentAchievement, ApplicationError> {
        let achievement = self
            .repo
            .find_by_id(command.achievement_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AchievementNotFound,
                    format!("Achievement {} not found", command.achievement_id),
                )
            })?;

        let existing = self
            .repo
            .find_by_student_and_achievement(command.student_id, command.achievement_id)
            .await?;

        if existing.is_some() {
            return Err(ApplicationError::Domain(DomainError::Validation(
                "Student already earned this achievement".to_string(),
            )));
        }

        let sa = StudentAchievement::new(
            command.tenant_id,
            command.student_id,
            command.achievement_id,
        );

        let event = AchievementEarned {
            student_achievement_id: sa.id,
            student_id: sa.student_id,
            achievement_id: sa.achievement_id,
            achievement_title: achievement.title.clone(),
            tenant_id: sa.tenant_id,
            metadata: EventMetadata::new(
                "learning.achievement.earned".to_string(),
                command.tenant_id,
                sa.id.to_string(),
                None,
                None,
                None,
                1,
                &crate::common::domain::clock::SystemClock,
            ),
        };

        self.repo.award(&sa).await?;
        let _ = self
            .event_bus
            .publish(Arc::from(Box::new(event) as Box<dyn DomainEvent>))
            .await;

        Ok(sa)
    }
}
