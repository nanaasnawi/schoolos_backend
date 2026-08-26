use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

use crate::common::error::ApplicationError;

type DynResource = dyn Any + Send + Sync;

#[derive(Clone, Debug)]
pub struct PermissionSnapshot {
    pub granted_permissions: std::collections::HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct AuthorizationContext {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
    pub roles: Vec<String>,
    pub snapshot: PermissionSnapshot,
}

/// A Requirement represents a single criteria that must be met (e.g., "MustBeTeacher", "MustOwnResource")
pub trait Requirement: Send + Sync {
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

/// A Handler evaluates a specific requirement.
#[async_trait]
pub trait AuthorizationHandler: Send + Sync {
    async fn handle(
        &self,
        context: &AuthorizationContext,
        requirement: &dyn Requirement,
        resource: Option<&DynResource>,
    ) -> Result<bool, ApplicationError>;
}

/// A Policy groups multiple requirements together.
pub struct Policy {
    pub name: String,
    pub requirements: Vec<Box<dyn Requirement>>,
}

impl Policy {
    pub fn new(name: impl Into<String>, requirements: Vec<Box<dyn Requirement>>) -> Self {
        Self {
            name: name.into(),
            requirements,
        }
    }
}

/// The AuthorizationService evaluates policies.
pub struct AuthorizationService {
    handlers: Vec<Box<dyn AuthorizationHandler>>,
    policies: HashMap<String, Policy>,
}

impl AuthorizationService {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            policies: HashMap::new(),
        }
    }

    pub fn register_handler(&mut self, handler: Box<dyn AuthorizationHandler>) {
        self.handlers.push(handler);
    }

    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.insert(policy.name.clone(), policy);
    }

    /// Starts a fluent authorization check for the given actor context.
    ///
    /// ```ignore
    /// authorization_service
    ///     .authorize(&context)
    ///     .can("EditStudent")
    ///     .resource(&student)
    ///     .await?;
    /// ```
    pub fn authorize<'a>(&'a self, context: &'a AuthorizationContext) -> AuthorizationBuilder<'a> {
        AuthorizationBuilder {
            service: self,
            context,
            policy_name: None,
            resource: None,
        }
    }

    pub async fn evaluate_policy(
        &self,
        policy_name: &str,
        context: &AuthorizationContext,
        resource: Option<&DynResource>,
    ) -> Result<bool, ApplicationError> {
        let policy = self.policies.get(policy_name).ok_or_else(|| {
            ApplicationError::Internal(format!("Policy '{}' not found", policy_name))
        })?;

        for requirement in &policy.requirements {
            let mut met = false;
            for handler in &self.handlers {
                if handler
                    .handle(context, requirement.as_ref(), resource)
                    .await?
                {
                    met = true;
                    break;
                }
            }
            if !met {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Default for AuthorizationService {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AuthorizationBuilder<'a> {
    service: &'a AuthorizationService,
    context: &'a AuthorizationContext,
    policy_name: Option<String>,
    resource: Option<&'a DynResource>,
}

impl<'a> AuthorizationBuilder<'a> {
    pub fn can(mut self, policy_name: impl Into<String>) -> Self {
        self.policy_name = Some(policy_name.into());
        self
    }

    pub fn resource<R: Any + Send + Sync>(mut self, resource: &'a R) -> Self {
        self.resource = Some(resource);
        self
    }
}

impl<'a> std::future::IntoFuture for AuthorizationBuilder<'a> {
    type Output = Result<bool, ApplicationError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let policy_name = self.policy_name.ok_or_else(|| {
                ApplicationError::Internal("Policy name not specified".to_string())
            })?;
            self.service
                .evaluate_policy(&policy_name, self.context, self.resource)
                .await
        })
    }
}
