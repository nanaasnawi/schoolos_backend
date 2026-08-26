use crate::permission::domain::permission_registry::Permission;
use crate::permission::domain::role::Role;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Actor {
    pub id: Uuid, // User ID
    pub tenant_id: Uuid,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>, // Flattened permissions from all roles
}

impl Actor {
    pub fn has_permission(&self, required_permission: &Permission) -> bool {
        self.permissions.contains(required_permission)
    }
}
