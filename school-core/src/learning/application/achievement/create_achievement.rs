use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::EventMetadata;
use crate::common::error::ApplicationError;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::achievement::{Achievement, AchievementCreated};
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateAchievementCommand {
    pub tenant_id: Uuid,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub criteria_type: String,
    pub criteria_value: String,
}

pub struct CreateAchievementUseCase {
    repo: Arc<dyn AchievementRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl CreateAchievementUseCase {
    pub fn new(
        repo: Arc<dyn AchievementRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: CreateAchievementCommand,
    ) -> Result<Achievement, ApplicationError> {
        let mut achievement = Achievement::new(
            command.tenant_id,
            command.title,
            command.description,
            command.icon,
            command.criteria_type,
            command.criteria_value,
            &*self.clock,
        );

        let event = AchievementCreated {
            achievement_id: achievement.id,
            title: achievement.title.clone(),
            tenant_id: achievement.tenant_id,
            metadata: EventMetadata::new(
                "learning.achievement.created".to_string(),
                achievement.tenant_id,
                achievement.id.to_string(),
                None,
                None,
                None,
                1,
                &*self.clock,
            ),
        };
        achievement.raise_event(event);

        self.repo.save(&achievement).await?;

        for e in achievement.take_events() {
            let _ = self.event_bus.publish(Arc::from(e)).await;
        }

        Ok(achievement)
    }
}
