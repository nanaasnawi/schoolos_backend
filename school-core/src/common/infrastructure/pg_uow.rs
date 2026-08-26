use super::uow::{UnitOfWork, UnitOfWorkFactory};
use crate::common::error::InfrastructureError;
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use std::any::Any;

pub struct PgUnitOfWork {
    tx: Option<Transaction<'static, Postgres>>,
}

impl PgUnitOfWork {
    pub fn new(tx: Transaction<'static, Postgres>) -> Self {
        Self { tx: Some(tx) }
    }
}

#[async_trait]
impl UnitOfWork for PgUnitOfWork {
    async fn commit(mut self: Box<Self>) -> Result<(), InfrastructureError> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await.map_err(InfrastructureError::Database)?;
        }
        Ok(())
    }

    async fn rollback(mut self: Box<Self>) -> Result<(), InfrastructureError> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await.map_err(InfrastructureError::Database)?;
        }
        Ok(())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self.tx.as_mut().unwrap()
    }
}

pub struct PgUnitOfWorkFactory {
    pool: PgPool,
}

impl PgUnitOfWorkFactory {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UnitOfWorkFactory for PgUnitOfWorkFactory {
    async fn begin(&self) -> Result<Box<dyn UnitOfWork>, InfrastructureError> {
        let tx = self
            .pool
            .begin()
            .await
            .map_err(InfrastructureError::Database)?;
        Ok(Box::new(PgUnitOfWork::new(tx)))
    }
}
