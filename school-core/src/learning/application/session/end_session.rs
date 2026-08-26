use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct EndSessionCommand {
    pub session_id: Uuid,
}

pub struct EndSessionUseCase {
    session_repo: Arc<dyn SessionRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl EndSessionUseCase {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            session_repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: EndSessionCommand,
    ) -> Result<LearningSession, ApplicationError> {
        let mut session = self
            .session_repo
            .find_by_id(command.session_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::SessionNotFound,
                    format!("Session {} not found", command.session_id),
                )
            })?;

        session.end(&*self.clock);

        self.session_repo.update(&session).await?;

        // Dispatch domain events via event bus
        for event in session.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(session)
    }
}
