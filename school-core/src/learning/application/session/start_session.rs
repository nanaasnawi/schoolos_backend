use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct StartSessionCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub class_id: Uuid,
    pub teacher_id: Uuid,
    pub notes: Option<String>,
}

pub struct StartSessionUseCase {
    session_repo: Arc<dyn SessionRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl StartSessionUseCase {
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
        command: StartSessionCommand,
    ) -> Result<LearningSession, ApplicationError> {
        let mut session = LearningSession::start_new(
            command.tenant_id,
            command.lesson_id,
            command.class_id,
            command.teacher_id,
            command.notes,
            &*self.clock,
        );

        self.session_repo.create(&session).await?;

        // Dispatch domain events via event bus
        for event in session.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(session)
    }
}
