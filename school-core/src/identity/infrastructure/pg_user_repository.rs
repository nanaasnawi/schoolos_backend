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
    async fn find_by_email_global(&self, email: &str) -> Result<Option<User>, InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, InfrastructureError>;
    async fn find_primary_role(&self, user_id: Uuid)
        -> Result<Option<String>, InfrastructureError>;
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
            INSERT INTO users (id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.username)
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
            SELECT u.id, u.tenant_id, u.username, u.email, u.password_hash, u.full_name, u.is_active, u.created_at, u.updated_at
            FROM users u
            LEFT JOIN teachers t ON t.user_id = u.id
            LEFT JOIN students s ON s.user_id = u.id
            LEFT JOIN guardians g ON g.user_id = u.id
            WHERE u.tenant_id = $1 
              AND (
                -- Admin and Super Admin accounts MUST strictly log in using their email
                (
                  EXISTS (
                    SELECT 1 FROM user_roles ur 
                    JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id 
                      AND (
                        LOWER(r.name) LIKE '%admin%' 
                        OR LOWER(r.name) LIKE '%kepala%' 
                        OR LOWER(r.name) LIKE '%operator%'
                      )
                  )
                  AND u.email ILIKE $2
                )
                OR
                -- Regular users (Guru, Siswa, Wali) can log in via name-based username, dotless username, email, NIP, NISN, phone, etc.
                (
                  NOT EXISTS (
                    SELECT 1 FROM user_roles ur 
                    JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id 
                      AND (
                        LOWER(r.name) LIKE '%admin%' 
                        OR LOWER(r.name) LIKE '%kepala%' 
                        OR LOWER(r.name) LIKE '%operator%'
                      )
                  )
                  AND (
                    u.email ILIKE $2 
                    OR u.username ILIKE $2
                    OR REPLACE(LOWER(COALESCE(u.username, '')), '.', '') = REPLACE(LOWER($2), '.', '')
                    OR u.full_name ILIKE $2
                    OR u.id::text = $2 
                    OR t.nip = $2 
                    OR s.nisn = $2 
                    OR g.phone_number = $2
                    OR g.phone_number = REPLACE($2, '+62', '0')
                    OR ('0' || SUBSTRING($2 FROM 3)) = g.phone_number
                    OR REPLACE(REPLACE(REPLACE(COALESCE(g.phone_number, ''), ' ', ''), '-', ''), '+62', '0') = REPLACE(REPLACE(REPLACE($2, ' ', ''), '-', ''), '+62', '0')
                  )
                )
              )
            ORDER BY 
              CASE 
                WHEN u.email ILIKE $2 THEN 1
                WHEN LOWER(COALESCE(u.username, '')) = LOWER($2) THEN 2
                WHEN REPLACE(LOWER(COALESCE(u.username, '')), '.', '') = REPLACE(LOWER($2), '.', '') THEN 3
                WHEN s.nisn = $2 OR t.nip = $2 THEN 4
                WHEN LOWER(u.full_name) = LOWER($2) THEN 5
                ELSE 6
              END ASC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_by_email_global(&self, email: &str) -> Result<Option<User>, InfrastructureError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT u.id, u.tenant_id, u.username, u.email, u.password_hash, u.full_name, u.is_active, u.created_at, u.updated_at
            FROM users u
            LEFT JOIN teachers t ON t.user_id = u.id
            LEFT JOIN students s ON s.user_id = u.id
            LEFT JOIN guardians g ON g.user_id = u.id
            WHERE u.is_active = true
              AND (
                -- Admin and Super Admin accounts MUST strictly log in using their email
                (
                  EXISTS (
                    SELECT 1 FROM user_roles ur 
                    JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id 
                      AND (
                        LOWER(r.name) LIKE '%admin%' 
                        OR LOWER(r.name) LIKE '%kepala%' 
                        OR LOWER(r.name) LIKE '%operator%'
                      )
                  )
                  AND u.email ILIKE $1
                )
                OR
                -- Regular users (Guru, Siswa, Wali) can log in via name-based username, dotless username, email, NIP, NISN, phone, etc.
                (
                  NOT EXISTS (
                    SELECT 1 FROM user_roles ur 
                    JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id 
                      AND (
                        LOWER(r.name) LIKE '%admin%' 
                        OR LOWER(r.name) LIKE '%kepala%' 
                        OR LOWER(r.name) LIKE '%operator%'
                      )
                  )
                  AND (
                    u.email ILIKE $1 
                    OR u.username ILIKE $1
                    OR REPLACE(LOWER(COALESCE(u.username, '')), '.', '') = REPLACE(LOWER($1), '.', '')
                    OR u.full_name ILIKE $1
                    OR u.id::text = $1 
                    OR t.nip = $1 
                    OR s.nisn = $1 
                    OR g.phone_number = $1
                    OR g.phone_number = REPLACE($1, '+62', '0')
                    OR ('0' || SUBSTRING($1 FROM 3)) = g.phone_number
                    OR REPLACE(REPLACE(REPLACE(COALESCE(g.phone_number, ''), ' ', ''), '-', ''), '+62', '0') = REPLACE(REPLACE(REPLACE($1, ' ', ''), '-', ''), '+62', '0')
                  )
                )
              )
            ORDER BY 
              CASE 
                WHEN u.email ILIKE $1 THEN 1
                WHEN LOWER(COALESCE(u.username, '')) = LOWER($1) THEN 2
                WHEN REPLACE(LOWER(COALESCE(u.username, '')), '.', '') = REPLACE(LOWER($1), '.', '') THEN 3
                WHEN s.nisn = $1 OR t.nip = $1 THEN 4
                WHEN LOWER(u.full_name) = LOWER($1) THEN 5
                ELSE 6
              END ASC,
              u.created_at DESC
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
            SELECT id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn find_primary_role(
        &self,
        user_id: Uuid,
    ) -> Result<Option<String>, InfrastructureError> {
        let role = sqlx::query_scalar::<_, String>(
            r#"
            SELECT r.name 
            FROM user_roles ur 
            JOIN roles r ON r.id = ur.role_id 
            WHERE ur.user_id = $1 
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(role)
    }
}
