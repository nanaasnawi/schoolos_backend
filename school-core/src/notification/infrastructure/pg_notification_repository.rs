use crate::common::error::InfrastructureError;
use crate::notification::domain::notification::Notification;
use crate::notification::domain::notification_channel::NotificationChannel;
use crate::notification::domain::notification_preference::NotificationPreference;
use crate::notification::infrastructure::repository_traits::{
    NotificationPreferenceRepository, NotificationRepository,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgNotificationRepository {
    pool: PgPool,
}

impl PgNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn channels_from_str(s: &str) -> Vec<NotificationChannel> {
    s.split(',')
        .filter_map(|c| NotificationChannel::from_str(c.trim()))
        .collect()
}

fn channels_to_str(channels: &[NotificationChannel]) -> String {
    channels
        .iter()
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[async_trait]
impl NotificationRepository for PgNotificationRepository {
    async fn create(&self, notification: &Notification) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, read_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(notification.id)
        .bind(notification.tenant_id)
        .bind(notification.user_id)
        .bind(&notification.title)
        .bind(&notification.body)
        .bind(&notification.notification_type)
        .bind(notification.channel.as_str())
        .bind(&notification.reference_type)
        .bind(notification.reference_id)
        .bind(notification.is_read)
        .bind(notification.read_at)
        .bind(notification.created_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Notification>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, read_at, created_at
               FROM notifications WHERE user_id = $1 AND tenant_id = $2
               ORDER BY created_at DESC
               LIMIT $3 OFFSET $4"#
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let channel_str: String = r.get("channel");
                Notification {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    user_id: r.get("user_id"),
                    title: r.get("title"),
                    body: r.get("body"),
                    notification_type: r.get("notification_type"),
                    channel: NotificationChannel::from_str(&channel_str)
                        .unwrap_or(NotificationChannel::InApp),
                    reference_type: r.get("reference_type"),
                    reference_id: r.get("reference_id"),
                    is_read: r.get("is_read"),
                    read_at: r.get("read_at"),
                    created_at: r.get("created_at"),
                    domain_events: Vec::new(),
                    version: 1,
                }
            })
            .collect();

        Ok(items)
    }

    async fn count_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError> {
        let record = sqlx::query(
            "SELECT COUNT(*) as count FROM notifications WHERE user_id = $1 AND tenant_id = $2",
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(record.get("count"))
    }

    async fn count_unread(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError> {
        let record = sqlx::query(
            "SELECT COUNT(*) as count FROM notifications WHERE user_id = $1 AND tenant_id = $2 AND is_read = FALSE"
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(record.get("count"))
    }

    async fn mark_read(&self, id: Uuid, user_id: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query(
            "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE id = $1 AND user_id = $2"
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn mark_all_read(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            "UPDATE notifications SET is_read = TRUE, read_at = NOW() WHERE user_id = $1 AND tenant_id = $2 AND is_read = FALSE"
        )
        .bind(user_id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl NotificationPreferenceRepository for PgNotificationRepository {
    async fn upsert(&self, pref: &NotificationPreference) -> Result<(), InfrastructureError> {
        let channels_str = channels_to_str(&pref.channels);
        sqlx::query(
            r#"
            INSERT INTO notification_preferences (id, tenant_id, user_id, notification_type, channels, is_enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (tenant_id, user_id, notification_type) DO UPDATE
            SET channels = $5, is_enabled = $6, updated_at = $8
            "#
        )
        .bind(pref.id)
        .bind(pref.tenant_id)
        .bind(pref.user_id)
        .bind(&pref.notification_type)
        .bind(&channels_str)
        .bind(pref.is_enabled)
        .bind(pref.created_at)
        .bind(pref.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_by_user(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<NotificationPreference>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, notification_type, channels, is_enabled, created_at, updated_at
               FROM notification_preferences WHERE user_id = $1 AND tenant_id = $2"#
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let channels_str: String = r.get("channels");
                NotificationPreference {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    user_id: r.get("user_id"),
                    notification_type: r.get("notification_type"),
                    channels: channels_from_str(&channels_str),
                    is_enabled: r.get("is_enabled"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                    domain_events: Vec::new(),
                    version: 1,
                }
            })
            .collect();

        Ok(items)
    }

    async fn find_by_user_and_type(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationPreference>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, notification_type, channels, is_enabled, created_at, updated_at
               FROM notification_preferences WHERE user_id = $1 AND tenant_id = $2 AND notification_type = $3"#
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(notification_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            let channels_str: String = r.get("channels");
            NotificationPreference {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                user_id: r.get("user_id"),
                notification_type: r.get("notification_type"),
                channels: channels_from_str(&channels_str),
                is_enabled: r.get("is_enabled"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            }
        }))
    }
}
