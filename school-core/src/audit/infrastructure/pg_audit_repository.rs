use crate::audit::domain::audit_log::AuditLog;
use crate::audit::infrastructure::repository_traits::AuditRepository;
use crate::common::error::InfrastructureError;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PgAuditRepository {
    pool: PgPool,
}

impl PgAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditRepository for PgAuditRepository {
    async fn log(&self, audit_log: &AuditLog) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, tenant_id, school_id, request_id, actor_id, action, resource,
                permission, policy, decision, reason, ip, user_agent, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(audit_log.id)
        .bind(audit_log.tenant_id)
        .bind(audit_log.school_id)
        .bind(&audit_log.request_id)
        .bind(audit_log.actor_id)
        .bind(&audit_log.action)
        .bind(&audit_log.resource)
        .bind(&audit_log.permission)
        .bind(&audit_log.policy)
        .bind(&audit_log.decision)
        .bind(&audit_log.reason)
        .bind(&audit_log.ip)
        .bind(&audit_log.user_agent)
        .bind(audit_log.timestamp)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }
}
