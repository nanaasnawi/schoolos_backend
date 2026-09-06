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
            SELECT u.id, u.tenant_id, u.email, u.password_hash, u.full_name, u.is_active, u.created_at, u.updated_at
            FROM users u
            LEFT JOIN teachers t ON t.user_id = u.id
            LEFT JOIN students s ON s.user_id = u.id
            LEFT JOIN guardians g ON g.user_id = u.id
            WHERE u.tenant_id = $1 
              AND (
                u.email ILIKE $2 
                OR u.id::text = $2 
                OR t.nip = $2 
                OR s.nisn = $2 
                OR g.phone_number = $2
                OR g.phone_number = REPLACE($2, '+62', '0')
                OR ('0' || SUBSTRING($2 FROM 3)) = g.phone_number
                OR REPLACE(REPLACE(REPLACE(COALESCE(g.phone_number, ''), ' ', ''), '-', ''), '+62', '0') = REPLACE(REPLACE(REPLACE($2, ' ', ''), '-', ''), '+62', '0')
              )
            LIMIT 1
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
            SELECT u.id, u.tenant_id, u.email, u.password_hash, u.full_name, u.is_active, u.created_at, u.updated_at
            FROM users u
            LEFT JOIN teachers t ON t.user_id = u.id
            LEFT JOIN students s ON s.user_id = u.id
            LEFT JOIN guardians g ON g.user_id = u.id
            WHERE (
                u.email ILIKE $1 
                OR u.id::text = $1 
                OR t.nip = $1 
                OR s.nisn = $1 
                OR g.phone_number = $1
                OR g.phone_number = REPLACE($1, '+62', '0')
                OR ('0' || SUBSTRING($1 FROM 3)) = g.phone_number
                OR REPLACE(REPLACE(REPLACE(COALESCE(g.phone_number, ''), ' ', ''), '-', ''), '+62', '0') = REPLACE(REPLACE(REPLACE($1, ' ', ''), '-', ''), '+62', '0')
            ) AND u.is_active = true
            ORDER BY u.created_at DESC
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
