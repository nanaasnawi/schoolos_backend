use crate::common::error::InfrastructureError;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::infrastructure::repository_traits::LearningMaterialRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgLearningMaterialRepository {
    pool: PgPool,
}

impl PgLearningMaterialRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LearningMaterialRepository for PgLearningMaterialRepository {
    async fn create(&self, material: &LearningMaterial) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO learning_materials (id, tenant_id, lesson_id, material_type, title, description, storage_key, external_url, order_index, visibility, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(material.id)
        .bind(material.tenant_id)
        .bind(material.lesson_id)
        .bind(&material.material_type)
        .bind(&material.title)
        .bind(&material.description)
        .bind(&material.storage_key)
        .bind(&material.external_url)
        .bind(material.order_index)
        .bind(&material.visibility)
        .bind(material.is_active)
        .bind(material.created_at)
        .bind(material.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<LearningMaterial>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, material_type, title, description, storage_key, external_url, order_index, visibility, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM learning_materials WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| LearningMaterial {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            material_type: r.get("material_type"),
            title: r.get("title"),
            description: r.get("description"),
            storage_key: r.get("storage_key"),
            external_url: r.get("external_url"),
            order_index: r.get("order_index"),
            visibility: r.get("visibility"),
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
    ) -> Result<Vec<LearningMaterial>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, material_type, title, description, storage_key, external_url, order_index, visibility, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM learning_materials WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY order_index ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| LearningMaterial {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                material_type: r.get("material_type"),
                title: r.get("title"),
                description: r.get("description"),
                storage_key: r.get("storage_key"),
                external_url: r.get("external_url"),
                order_index: r.get("order_index"),
                visibility: r.get("visibility"),
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

    async fn update(&self, material: &LearningMaterial) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE learning_materials
            SET title = $1, description = $2, storage_key = $3, external_url = $4, visibility = $5, updated_at = NOW()
            WHERE id = $6 AND tenant_id = $7 AND deleted_at IS NULL
            "#
        )
        .bind(&material.title)
        .bind(&material.description)
        .bind(&material.storage_key)
        .bind(&material.external_url)
        .bind(&material.visibility)
        .bind(material.id)
        .bind(material.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE learning_materials
            SET deleted_at = NOW(), deleted_by = $1, is_active = false
            WHERE id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(deleted_by)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }
}
