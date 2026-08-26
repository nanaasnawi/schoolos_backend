use crate::audit::domain::audit_log::PolicyDecision;
use async_trait::async_trait;

pub struct ResourceContext {
    pub target_resource_id: Option<uuid::Uuid>,
    pub is_academic_year_active: bool,
    pub actor_id: uuid::Uuid,
}

#[async_trait]
pub trait Policy: Send + Sync {
    async fn evaluate(&self, context: &ResourceContext) -> PolicyDecision;
}

pub struct ActiveAcademicYearPolicy;

#[async_trait]
impl Policy for ActiveAcademicYearPolicy {
    async fn evaluate(&self, context: &ResourceContext) -> PolicyDecision {
        if context.is_academic_year_active {
            PolicyDecision {
                allowed: true,
                reason: None,
                policy_name: "ActiveAcademicYearPolicy".to_string(),
            }
        } else {
            PolicyDecision {
                allowed: false,
                reason: Some(
                    "Academic Year is currently closed. Operations are not allowed.".to_string(),
                ),
                policy_name: "ActiveAcademicYearPolicy".to_string(),
            }
        }
    }
}
