use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::common::domain::outbox::{OutboxEvent, OutboxRepository, OutboxStatus};
use crate::common::error::InfrastructureError;
use crate::common::infrastructure::uow::UnitOfWork;

pub struct PgOutboxRepository {
    pool: PgPool,
}

impl PgOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepository for PgOutboxRepository {
    async fn save(
        &self,
        event: &OutboxEvent,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<Transaction<'static, Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            INSERT INTO outbox_events (id, event_type, payload, status, created_at, processed_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(event.id)
        .bind(&event.event_type)
        .bind(&event.payload)
        .bind(event.status.as_db_str())
        .bind(event.created_at)
        .bind(event.processed_at)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn get_pending_events(
        &self,
        limit: i64,
    ) -> Result<Vec<OutboxEvent>, InfrastructureError> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, payload, status, created_at, processed_at
            FROM outbox_events
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(rows
            .into_iter()
            .map(|row| OutboxEvent {
                id: row.get("id"),
                event_type: row.get("event_type"),
                payload: row.get("payload"),
                status: OutboxStatus::from_db_str(row.get::<String, _>("status").as_str()),
                created_at: row.get("created_at"),
                processed_at: row.get("processed_at"),
            })
            .collect())
    }

    async fn update_event_status(&self, event: &OutboxEvent) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE outbox_events
            SET status = $2, processed_at = $3
            WHERE id = $1
            "#,
        )
        .bind(event.id)
        .bind(event.status.as_db_str())
        .bind(event.processed_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }
}
