use crate::audit::domain::audit_log::{AuditLog, AuthorizationDecision};
use crate::audit::infrastructure::repository_traits::AuditRepository;
use crate::common::domain::clock::Clock;
use crate::common::error::InfrastructureError;
use std::sync::Arc;
use uuid::Uuid;

pub struct AuditService {
    repository: Arc<dyn AuditRepository>,
    clock: Arc<dyn Clock>,
}

impl AuditService {
    pub fn new(repository: Arc<dyn AuditRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn log_access(
        &self,
        tenant_id: Uuid,
        school_id: Option<Uuid>,
        request_id: Option<String>,
        actor_id: Option<Uuid>,
        action: String,
        resource: Option<String>,
        decision: AuthorizationDecision,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), InfrastructureError> {
        let audit_log = AuditLog::new(
            tenant_id,
            school_id,
            request_id,
            actor_id,
            action,
            resource,
            decision,
            ip,
            user_agent,
            &*self.clock,
        );
        self.repository.log(&audit_log).await
    }
}
