use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub trait DomainEvent: Send + Sync + std::fmt::Debug {
    fn event_name(&self) -> &str;
    fn metadata(&self) -> &EventMetadata;
    fn to_json_value(&self) -> serde_json::Value;
}

/// Identifies the system component that produced the event.
/// Critical for distributed tracing and event replay debugging.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    #[default]
    ApiServer,
    Scheduler,
    BackgroundWorker,
    Migration,
    System,
}

impl std::fmt::Display for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ApiServer => "api-server",
            Self::Scheduler => "scheduler",
            Self::BackgroundWorker => "background-worker",
            Self::Migration => "migration",
            Self::System => "system",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub request_id: Option<String>,
    pub correlation_id: String,
    pub causation_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub version: u32,
    /// The system component that generated this event.
    pub source: EventSource,
}

impl EventMetadata {
    pub fn new(
        event_type: String,
        tenant_id: Uuid,
        correlation_id: String,
        request_id: Option<String>,
        causation_id: Option<Uuid>,
        actor_id: Option<Uuid>,
        version: u32,
        clock: &dyn Clock,
    ) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            event_type,
            occurred_at: clock.now(),
            request_id,
            correlation_id,
            causation_id,
            tenant_id,
            actor_id,
            version,
            source: EventSource::default(), // Default to api-server
        }
    }

    pub fn with_source(mut self, source: EventSource) -> Self {
        self.source = source;
        self
    }
}
