use crate::common::error::InfrastructureError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: uuid::Uuid,
    pub job_type: String,
    pub payload: serde_json::Value,
    pub retries: u32,
    pub max_retry: u32,
    pub next_retry: chrono::DateTime<chrono::Utc>,
    pub backoff_multiplier: f32,
    pub is_dead_letter: bool,
    pub run_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait JobQueue: Send + Sync {
    async fn enqueue(&self, job: Job) -> Result<(), InfrastructureError>;
    async fn dequeue(&self, job_types: &[String]) -> Result<Option<Job>, InfrastructureError>;
    async fn complete(&self, job_id: uuid::Uuid) -> Result<(), InfrastructureError>;
    async fn fail(&self, job_id: uuid::Uuid, error: &str) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait JobHandler: Send + Sync {
    fn job_type(&self) -> &str;
    async fn handle(&self, job: &Job) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
