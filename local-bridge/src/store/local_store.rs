use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::time::Duration;
use tracing::info;

pub struct LocalStore {
    pool: Pool<Sqlite>,
}

impl LocalStore {
    pub async fn new(db_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(db_url)
            .await?;

        let store = Self { pool };
        store.run_migrations().await?;
        
        Ok(store)
    }

    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        info!("Running local SQLite migrations...");
        
        // 1. Encrypted Local Outbox Queue
        // For PUSH mechanism (Cloud -> Dapodik)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS local_outbox (
                id TEXT PRIMARY KEY,
                idempotency_key TEXT NOT NULL UNIQUE,
                tenant_id TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                operation TEXT NOT NULL,
                payload_encrypted TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'PENDING',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                processed_at DATETIME
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 2. Local Audit Store
        // For offline tracking (Agent -> Cloud)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS local_audit_events (
                event_id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                correlation_id TEXT,
                source TEXT NOT NULL,
                payload TEXT NOT NULL,
                occurred_at DATETIME NOT NULL,
                sync_status TEXT NOT NULL DEFAULT 'UNSYNCED',
                synced_at DATETIME
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 3. Sync Cursors
        // For tracking the last synchronized state per entity
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sync_cursors (
                entity_type TEXT PRIMARY KEY,
                last_cursor INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // 4. Entity Snapshots for Reconciliation
        // Stores the hash of the last successfully synced payload to detect changes locally
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entity_snapshots (
                entity_type TEXT NOT NULL,
                external_id TEXT NOT NULL,
                payload_hash TEXT NOT NULL,
                synced_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (entity_type, external_id)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("Local SQLite migrations completed successfully.");
        Ok(())
    }

    pub async fn get_cursor(&self, entity_type: &str) -> Result<i64, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT last_cursor FROM sync_cursors WHERE entity_type = ?"
        )
        .bind(entity_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(c,)| c).unwrap_or(0))
    }

    pub async fn set_cursor(&self, entity_type: &str, cursor: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO sync_cursors (entity_type, last_cursor, updated_at) 
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(entity_type) 
            DO UPDATE SET last_cursor = excluded.last_cursor, updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(entity_type)
        .bind(cursor)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_snapshot_hash(&self, entity_type: &str, external_id: &str) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT payload_hash FROM entity_snapshots WHERE entity_type = ? AND external_id = ?"
        )
        .bind(entity_type)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(h,)| h))
    }

    pub async fn save_snapshot_hash(&self, entity_type: &str, external_id: &str, hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO entity_snapshots (entity_type, external_id, payload_hash, synced_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(entity_type, external_id)
            DO UPDATE SET payload_hash = excluded.payload_hash, synced_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(entity_type)
        .bind(external_id)
        .bind(hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
