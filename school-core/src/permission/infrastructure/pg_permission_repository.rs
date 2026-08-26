use crate::common::error::InfrastructureError;
use crate::permission::domain::permission_registry::Permission;
use crate::permission::domain::role::Role;
use crate::permission::infrastructure::repository_traits::RoleRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgRoleRepository {
    pool: PgPool,
}

impl PgRoleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoleRepository for PgRoleRepository {
    async fn create(&self, role: &Role) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO roles (id, tenant_id, name, is_system_default, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(role.id)
        .bind(role.tenant_id)
        .bind(&role.name)
        .bind(role.is_system_default)
        .bind(role.created_at)
        .bind(role.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn assign_permissions(
        &self,
        role_id: Uuid,
        permissions: Vec<Permission>,
    ) -> Result<(), InfrastructureError> {
        // Simple approach: Delete existing, insert new
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(InfrastructureError::Database)?;

        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await
            .map_err(InfrastructureError::Database)?;

        for perm in permissions {
            sqlx::query("INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)")
                .bind(role_id)
                .bind(perm.as_str())
                .execute(&mut *tx)
                .await
                .map_err(InfrastructureError::Database)?;
        }

        tx.commit().await.map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn get_role_permissions(
        &self,
        role_id: Uuid,
    ) -> Result<Vec<Permission>, InfrastructureError> {
        let records = sqlx::query("SELECT permission FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        let mut permissions = Vec::new();
        for row in records {
            let perm_str: String = row.get("permission");
            if let Some(perm) = Permission::from_str(&perm_str) {
                permissions.push(perm);
            }
        }
        Ok(permissions)
    }

    async fn get_roles_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Role>, InfrastructureError> {
        let records = sqlx::query("SELECT id, tenant_id, name, is_system_default, created_at, updated_at FROM roles WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| Role {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                name: r.get("name"),
                is_system_default: r.get("is_system_default"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    async fn find_roles_by_user_id(&self, user_id: Uuid) -> Result<Vec<Role>, InfrastructureError> {
        let records = sqlx::query(
            r#"
            SELECT r.id, r.tenant_id, r.name, r.is_system_default, r.created_at, r.updated_at
            FROM roles r
            INNER JOIN user_roles ur ON ur.role_id = r.id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| Role {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                name: r.get("name"),
                is_system_default: r.get("is_system_default"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }
}
