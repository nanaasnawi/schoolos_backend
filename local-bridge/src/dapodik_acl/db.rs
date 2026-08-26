use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::time::Duration;
use tracing::info;

pub struct DapodikDb {
    pool: Pool<Postgres>,
}

impl DapodikDb {
    pub async fn new(db_url: &str) -> Result<Self, sqlx::Error> {
        info!("Connecting to Dapodik Local Postgres database...");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(db_url)
            .await?;
            
        Ok(Self { pool })
    }

    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}
