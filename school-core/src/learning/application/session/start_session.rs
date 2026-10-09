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
    pub session_type: Option<String>,
    pub schedule_id: Option<Uuid>,
    pub lesson_id: Option<Uuid>,
    pub class_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Uuid,
    pub session_date: Option<chrono::NaiveDate>,
    pub session_number: Option<i32>,
    pub start_time: Option<chrono::NaiveTime>,
    pub end_time: Option<chrono::NaiveTime>,
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
        let now_date = self.clock.now().date_naive();
        let session_date = command.session_date.unwrap_or(now_date);
        let session_type = command.session_type.unwrap_or_else(|| {
            if command.schedule_id.is_some() {
                "scheduled".to_string()
            } else {
                "adhoc".to_string()
            }
        });

        let mut session = LearningSession::start_new(
            command.tenant_id,
            session_type,
            command.schedule_id,
            command.lesson_id,
            command.class_id,
            command.subject_id,
            command.teacher_id,
            session_date,
            command.session_number.unwrap_or(1),
            command.start_time,
            command.end_time,
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
