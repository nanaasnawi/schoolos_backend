use crate::common::error::InfrastructureError;
use sqlx::{PgConnection, PgPool};
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait TransactionManager: Send + Sync {
    fn execute_in_transaction<'a, T, E, F>(&'a self, f: F) -> BoxFuture<'a, Result<T, E>>
    where
        T: Send + 'a,
        E: From<InfrastructureError> + Send + 'a,
        F: for<'c> FnOnce(&'c mut PgConnection) -> BoxFuture<'c, Result<T, E>> + Send + 'a;
}

pub struct PgTransactionManager {
    pool: PgPool,
}

impl PgTransactionManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl TransactionManager for PgTransactionManager {
    fn execute_in_transaction<'a, T, E, F>(&'a self, f: F) -> BoxFuture<'a, Result<T, E>>
    where
        T: Send + 'a,
        E: From<InfrastructureError> + Send + 'a,
        F: for<'c> FnOnce(&'c mut PgConnection) -> BoxFuture<'c, Result<T, E>> + Send + 'a,
    {
        Box::pin(async move {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(InfrastructureError::Database)?;

            match f(&mut tx).await {
                Ok(val) => {
                    tx.commit().await.map_err(InfrastructureError::Database)?;
                    Ok(val)
                }
                Err(e) => {
                    let _ = tx.rollback().await; // Ignore rollback error if it fails
                    Err(e)
                }
            }
        })
    }
}
