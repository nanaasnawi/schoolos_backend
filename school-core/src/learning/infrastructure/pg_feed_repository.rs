use crate::common::error::InfrastructureError;
use crate::learning::domain::classroom_feed::FeedItem;
use crate::learning::infrastructure::repository_traits::FeedRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgFeedRepository {
    pool: PgPool,
}

impl PgFeedRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FeedRepository for PgFeedRepository {
    async fn create(&self, item: &FeedItem) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO classroom_feeds (id, tenant_id, class_id, actor_id, actor_name, action, target_type, target_id, summary, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(item.id)
        .bind(item.tenant_id)
        .bind(item.class_id)
        .bind(item.actor_id)
        .bind(&item.actor_name)
        .bind(&item.action)
        .bind(&item.target_type)
        .bind(item.target_id)
        .bind(&item.summary)
        .bind(&item.metadata)
        .bind(item.created_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_class(
        &self,
        class_id: Uuid,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItem>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, class_id, actor_id, actor_name, action, target_type, target_id, summary, metadata, created_at
               FROM classroom_feeds WHERE class_id = $1 AND tenant_id = $2
               ORDER BY created_at DESC
               LIMIT $3 OFFSET $4"#
        )
        .bind(class_id)
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| FeedItem {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                class_id: r.get("class_id"),
                actor_id: r.get("actor_id"),
                actor_name: r.get("actor_name"),
                action: r.get("action"),
                target_type: r.get("target_type"),
                target_id: r.get("target_id"),
                summary: r.get("summary"),
                metadata: r.get("metadata"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(items)
    }

    async fn count_by_class(
        &self,
        class_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError> {
        let record = sqlx::query(
            "SELECT COUNT(*) as count FROM classroom_feeds WHERE class_id = $1 AND tenant_id = $2",
        )
        .bind(class_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.get("count"))
    }
}
