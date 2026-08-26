use crate::common::error::InfrastructureError;
use crate::permission::domain::permission_registry::Permission;
use crate::permission::domain::role::Role;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn create(&self, role: &Role) -> Result<(), InfrastructureError>;
    async fn assign_permissions(
        &self,
        role_id: Uuid,
        permissions: Vec<Permission>,
    ) -> Result<(), InfrastructureError>;
    async fn get_role_permissions(
        &self,
        role_id: Uuid,
    ) -> Result<Vec<Permission>, InfrastructureError>;
    async fn get_roles_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Role>, InfrastructureError>;
    async fn find_roles_by_user_id(&self, user_id: Uuid) -> Result<Vec<Role>, InfrastructureError>;
}
