use crate::common::error::InfrastructureError;
use async_trait::async_trait;
use std::any::Any;

#[async_trait]
pub trait UnitOfWork: Send {
    async fn commit(self: Box<Self>) -> Result<(), InfrastructureError>;
    async fn rollback(self: Box<Self>) -> Result<(), InfrastructureError>;

    /// Provides downcasting to the underlying transaction (e.g. `&mut sqlx::Transaction`).
    fn as_any(&mut self) -> &mut dyn Any;
}

#[async_trait]
pub trait UnitOfWorkFactory: Send + Sync {
    async fn begin(&self) -> Result<Box<dyn UnitOfWork>, InfrastructureError>;
}
