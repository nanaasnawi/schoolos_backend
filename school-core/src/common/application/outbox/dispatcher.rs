use std::sync::Arc;

use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use crate::common::domain::outbox::{OutboxEvent, OutboxRepository, OutboxStatus};
use crate::common::event_bus::SharedEventBus;

/// Polls the transactional outbox and publishes domain events to the integration event bus.
pub struct OutboxDispatcher {
    repo: Arc<dyn OutboxRepository>,
    event_bus: SharedEventBus,
    clock: Arc<dyn Clock>,
    poll_interval: Duration,
}

impl OutboxDispatcher {
    pub fn new(
        repo: Arc<dyn OutboxRepository>,
        event_bus: SharedEventBus,
        clock: Arc<dyn Clock>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            repo,
            event_bus,
            clock,
            poll_interval,
        }
    }

    pub async fn start(&self) {
        info!("Starting Outbox Dispatcher...");
        loop {
            match self.repo.get_pending_events(100).await {
                Ok(events) => {
                    if events.is_empty() {
                        sleep(self.poll_interval).await;
                        continue;
                    }

                    for mut event in events {
                        if let Err(e) = self.process_event(&mut event).await {
                            error!("Failed to process outbox event {}: {:?}", event.id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Error fetching outbox events: {:?}", e);
                    sleep(self.poll_interval).await;
                }
            }
        }
    }

    async fn process_event(
        &self,
        event: &mut OutboxEvent,
    ) -> Result<(), crate::common::error::InfrastructureError> {
        event.status = OutboxStatus::Processing;
        event.processed_at = Some(self.clock.now());
        self.repo.update_event_status(event).await?;

        let domain_event: Arc<dyn DomainEvent> =
            Arc::new(GenericOutboxEvent::from_outbox(event, &*self.clock));
        self.event_bus.publish(domain_event).await;
        info!(
            "Published outbox event {} of type {} to event bus",
            event.id, event.event_type
        );

        event.status = OutboxStatus::Succeeded;
        event.processed_at = Some(self.clock.now());
        self.repo.update_event_status(event).await?;

        Ok(())
    }
}

/// Rehydrated domain event from the transactional outbox for integration publishing.
#[derive(Debug, Clone)]
pub struct GenericOutboxEvent {
    event_type: String,
    payload: serde_json::Value,
    metadata: crate::common::domain::event::EventMetadata,
}

impl GenericOutboxEvent {
    pub fn from_outbox(event: &OutboxEvent, clock: &dyn Clock) -> Self {
        use crate::common::domain::event::EventSource;

        let metadata = event
            .payload
            .get("metadata")
            .and_then(|value| {
                serde_json::from_value::<crate::common::domain::event::EventMetadata>(value.clone())
                    .ok()
            })
            .unwrap_or_else(|| {
                crate::common::domain::event::EventMetadata::new(
                    event.event_type.clone(),
                    uuid::Uuid::nil(),
                    event.id.to_string(),
                    None,
                    None,
                    None,
                    1,
                    clock,
                )
                .with_source(EventSource::BackgroundWorker)
            });

        Self {
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
            metadata,
        }
    }
}

impl DomainEvent for GenericOutboxEvent {
    fn event_name(&self) -> &str {
        &self.event_type
    }

    fn metadata(&self) -> &crate::common::domain::event::EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        self.payload.clone()
    }
}
