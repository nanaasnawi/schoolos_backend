use crate::audit::domain::audit_log::AuditLog;
use crate::common::error::InfrastructureError;
use async_trait::async_trait;

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn log(&self, audit_log: &AuditLog) -> Result<(), InfrastructureError>;
}
