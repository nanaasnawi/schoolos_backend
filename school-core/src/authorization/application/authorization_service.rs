use crate::audit::domain::audit_log::AuthorizationDecision;
use crate::authorization::domain::actor::Actor;
use crate::permission::domain::permission_registry::Permission;
use crate::policy::domain::policy_engine::{Policy, ResourceContext};
use std::sync::Arc;

pub struct AuthorizationService {
    policies: Vec<Arc<dyn Policy>>,
}

impl AuthorizationService {
    pub fn new(policies: Vec<Arc<dyn Policy>>) -> Self {
        Self { policies }
    }

    pub async fn authorize(
        &self,
        actor: &Actor,
        action: Permission,
        context: &ResourceContext,
    ) -> AuthorizationDecision {
        // 1. Check explicit Role Permission
        if !actor.has_permission(&action) {
            return AuthorizationDecision {
                allowed: false,
                permission: action.clone(),
                policy: None,
                reason: Some(format!(
                    "Actor does not have the required explicit permission: {:?}",
                    action
                )),
            };
        }

        // 2. Evaluate Contextual Policies
        for policy in &self.policies {
            let decision = policy.evaluate(context).await;
            if !decision.allowed {
                // Return immediately upon first policy denial
                return AuthorizationDecision {
                    allowed: false,
                    permission: action.clone(),
                    policy: Some(decision.policy_name),
                    reason: decision.reason,
                };
            }
        }

        AuthorizationDecision {
            allowed: true,
            permission: action,
            policy: None,
            reason: None,
        }
    }
}
