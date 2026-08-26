use crate::common::domain::clock::Clock;
use crate::permission::domain::permission_registry::Permission;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: Option<String>,
    pub policy_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationDecision {
    pub allowed: bool,
    pub permission: Permission,
    pub policy: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub school_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub resource: Option<String>,
    pub permission: String,
    pub policy: Option<String>,
    pub decision: String,
    pub reason: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AuditLog {
    pub fn new(
        tenant_id: Uuid,
        school_id: Option<Uuid>,
        request_id: Option<String>,
        actor_id: Option<Uuid>,
        action: String,
        resource: Option<String>,
        decision: AuthorizationDecision,
        ip: Option<String>,
        user_agent: Option<String>,
        clock: &dyn Clock,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            school_id,
            request_id,
            actor_id,
            action,
            resource,
            permission: decision.permission.as_str().to_string(),
            policy: decision.policy,
            decision: if decision.allowed {
                "Allowed".to_string()
            } else {
                "Denied".to_string()
            },
            reason: decision.reason,
            ip,
            user_agent,
            timestamp: clock.now(),
        }
    }
}
