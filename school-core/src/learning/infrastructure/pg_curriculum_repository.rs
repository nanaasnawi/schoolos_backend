use crate::common::error::InfrastructureError;
use crate::learning::domain::curriculum::Curriculum;
use crate::learning::infrastructure::repository_traits::CurriculumRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgCurriculumRepository {
    pool: PgPool,
}

impl PgCurriculumRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CurriculumRepository for PgCurriculumRepository {
    async fn create(&self, curriculum: &Curriculum) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO curricula (id, tenant_id, code, name, description, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(curriculum.id)
        .bind(curriculum.tenant_id)
        .bind(&curriculum.code)
        .bind(&curriculum.name)
        .bind(&curriculum.description)
        .bind(curriculum.is_active)
        .bind(curriculum.created_at)
        .bind(curriculum.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Curriculum>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, code, name, description, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM curricula WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Curriculum {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            code: r.get("code"),
            name: r.get("name"),
            description: r.get("description"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<Curriculum>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, code, name, description, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM curricula WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY code ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Curriculum {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                code: r.get("code"),
                name: r.get("name"),
                description: r.get("description"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(items)
    }
}
