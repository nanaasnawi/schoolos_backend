use crate::common::error::InfrastructureError;
use crate::identity::domain::tenant::Tenant;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn create(&self, tenant: &Tenant) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tenant>, InfrastructureError>;
}

pub struct PgTenantRepository {
    pool: PgPool,
}

impl PgTenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
#[derive(sqlx::FromRow)]
struct PgTenantRecord {
    id: Uuid,
    name: String,
    domain: Option<String>,
    is_active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<PgTenantRecord> for Tenant {
    fn from(r: PgTenantRecord) -> Self {
        Tenant {
            id: r.id,
            name: r.name,
            domain: r.domain,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            domain_events: Vec::new(),
            version: 1,
        }
    }
}

impl TenantRepository for PgTenantRepository {
    async fn create(&self, tenant: &Tenant) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO tenants (id, name, domain, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(tenant.id)
        .bind(&tenant.name)
        .bind(&tenant.domain)
        .bind(tenant.is_active)
        .bind(tenant.created_at)
        .bind(tenant.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Tenant>, InfrastructureError> {
        let tenant = sqlx::query_as::<_, PgTenantRecord>(
            r#"
            SELECT id, name, domain, is_active, created_at, updated_at
            FROM tenants
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .map(Into::into);

        Ok(tenant)
    }
}
