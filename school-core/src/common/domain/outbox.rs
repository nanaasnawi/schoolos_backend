use crate::common::error::InfrastructureError;
use async_trait::async_trait;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    DeadLetter,
}

impl OutboxStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
        }
    }

    pub fn from_db_str(value: &str) -> Self {
        match value {
            "processing" => Self::Processing,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "dead_letter" => Self::DeadLetter,
            _ => Self::Pending,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OutboxEvent {
    pub id: uuid::Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: OutboxStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub processed_at: Option<chrono::DateTime<chrono::Utc>>,
}

use crate::common::infrastructure::uow::UnitOfWork;

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn save(
        &self,
        event: &OutboxEvent,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;

    async fn get_pending_events(&self, limit: i64)
        -> Result<Vec<OutboxEvent>, InfrastructureError>;

    async fn update_event_status(&self, event: &OutboxEvent) -> Result<(), InfrastructureError>;
}
