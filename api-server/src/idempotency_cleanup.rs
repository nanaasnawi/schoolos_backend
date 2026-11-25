use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

use sqlx::PgPool;

#[derive(Clone)]
pub struct IdempotencyCleanupService {
    pool: PgPool,
    cleanup_interval: Duration,
    ttl: Duration,
}

impl IdempotencyCleanupService {
    pub fn new(pool: sqlx::PgPool, cleanup_interval: Duration, ttl: Duration) -> Self {
        Self {
            pool,
            cleanup_interval,
            ttl,
        }
    }

    pub async fn start(&self) {
        info!(
            component = "idempotency_cleanup",
            cleanup_interval_sec = self.cleanup_interval.as_secs(),
            ttl_sec = self.ttl.as_secs(),
            "Idempotency Cleanup Service started"
        );

        let mut timer = interval(self.cleanup_interval);
        loop {
            timer.tick().await;
            if let Err(e) = self.cleanup().await {
                error!(
                    error = ?e,
                    "Failed to cleanup idempotency keys"
                );
            }
        }
    }

    async fn cleanup(&self) -> Result<(), sqlx::Error> {
        let chrono_ttl =
            chrono::Duration::from_std(self.ttl).unwrap_or_else(|_| chrono::Duration::days(1));
        let cutoff = sqlx::types::chrono::Utc::now()
            .checked_sub_signed(chrono_ttl)
            .unwrap_or_default();

        let mut total_deleted: u64 = 0;
        loop {
            let deleted = crate::db_tenant::with_system_rls_bypass(async {
                sqlx::query(
                    r#"
                    DELETE FROM idempotency_keys
                    WHERE idempotency_key IN (
                        SELECT idempotency_key
                        FROM idempotency_keys
                        WHERE created_at < $1
                        ORDER BY created_at ASC
                        LIMIT 5000
                    )
                    "#,
                )
                .bind(cutoff)
                .execute(&self.pool)
                .await
            })
            .await?;

            let count = deleted.rows_affected();
            total_deleted += count;
            if count < 5000 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if total_deleted > 0 {
            info!(
                deleted_count = total_deleted,
                cutoff = ?cutoff,
                "Idempotency keys batched cleanup completed"
            );
        }

        Ok(())
    }
}
