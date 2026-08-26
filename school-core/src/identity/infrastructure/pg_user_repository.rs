use crate::common::error::InfrastructureError;
use crate::identity::domain::user::User;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<(), InfrastructureError>;
    async fn find_by_email(
        &self,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<User>, InfrastructureError>;
    // Lookup by email ONLY (no tenant filter) — used for global login flow
    async fn find_by_email_global(
        &self,
        email: &str,
    ) -> Result<Option<User>, InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, InfrastructureError>;
}

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create(&self, user: &User) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(&user.full_name)
        .bind(user.is_active)
        .bind(user.created_at)
        .bind(user.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_email(
        &self,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<User>, InfrastructureError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at
            FROM users
            WHERE tenant_id = $1 AND email = $2
            "#,
        )
        .bind(tenant_id)
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_by_email_global(
        &self,
        email: &str,
    ) -> Result<Option<User>, InfrastructureError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at
            FROM users
            WHERE email = $1 AND is_active = true
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, InfrastructureError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }
}
