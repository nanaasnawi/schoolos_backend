use crate::audit::domain::audit_log::{AuditLog, AuthorizationDecision};
use crate::audit::infrastructure::repository_traits::AuditRepository;
use crate::common::domain::clock::Clock;
use crate::common::event_bus::InMemoryEventBus;
use crate::permission::domain::permission_registry::Permission;
use std::sync::Arc;
use tracing::{error, info};

pub struct AuditEventSubscriber;

impl AuditEventSubscriber {
    pub fn start(
        event_bus: Arc<InMemoryEventBus>,
        audit_repo: Arc<dyn AuditRepository>,
        clock: Arc<dyn Clock>,
    ) {
        let mut receiver = event_bus.subscribe();

        tokio::spawn(async move {
            info!(
                component = "audit_engine",
                "Audit Engine started — listening for domain events"
            );

            while let Ok(event) = receiver.recv().await {
                let metadata = event.metadata();
                let event_id = metadata.event_id;
                let event_name = event.event_name();
                let request_id = metadata.request_id.clone();
                let tenant_id = metadata.tenant_id;
                let source = metadata.source.to_string();

                let decision = AuthorizationDecision {
                    allowed: true,
                    permission: Permission::SystemInternal,
                    policy: Some("DomainEventSubscription".to_string()),
                    reason: Some(format!("Domain event received from source={}", source)),
                };

                let audit_log = AuditLog::new(
                    tenant_id,
                    None, // school_id
                    request_id,
                    metadata.actor_id,
                    event_name.to_string(),
                    Some(event_id.to_string()),
                    decision,
                    None, // ip
                    None, // user_agent
                    &*clock,
                );

                match audit_repo.log(&audit_log).await {
                    Ok(_) => {
                        info!(
                            event_id = %event_id,
                            event_name = event_name,
                            tenant_id = %tenant_id,
                            source = source,
                            "Audit log persisted"
                        );
                    }
                    Err(e) => {
                        // Best-effort: audit failure must NOT propagate to the business transaction
                        error!(
                            event_id = %event_id,
                            event_name = event_name,
                            error = ?e,
                            "Failed to persist audit log — best-effort, business transaction unaffected"
                        );
                    }
                }
            }

            info!(component = "audit_engine", "Audit Engine stopped");
        });
    }
}
