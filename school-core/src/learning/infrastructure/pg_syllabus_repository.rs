use crate::common::error::InfrastructureError;
use crate::learning::domain::syllabus::Syllabus;
use crate::learning::domain::syllabus_competency::SyllabusCompetency;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgSyllabusRepository {
    pool: PgPool,
}

impl PgSyllabusRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SyllabusRepository for PgSyllabusRepository {
    async fn create(&self, syllabus: &Syllabus) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO syllabuses (id, tenant_id, curriculum_id, subject_id, grade_level_id, code, name, description, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(syllabus.id)
        .bind(syllabus.tenant_id)
        .bind(syllabus.curriculum_id)
        .bind(syllabus.subject_id)
        .bind(syllabus.grade_level_id)
        .bind(&syllabus.code)
        .bind(&syllabus.name)
        .bind(&syllabus.description)
        .bind(syllabus.is_active)
        .bind(syllabus.created_at)
        .bind(syllabus.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Syllabus>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, curriculum_id, subject_id, grade_level_id, code, name, description, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM syllabuses WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Syllabus {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            curriculum_id: r.get("curriculum_id"),
            subject_id: r.get("subject_id"),
            grade_level_id: r.get("grade_level_id"),
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

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Syllabus>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, curriculum_id, subject_id, grade_level_id, code, name, description, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM syllabuses WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY code ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Syllabus {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                curriculum_id: r.get("curriculum_id"),
                subject_id: r.get("subject_id"),
                grade_level_id: r.get("grade_level_id"),
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

    async fn add_competency(
        &self,
        competency: &SyllabusCompetency,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO syllabus_competencies (id, tenant_id, syllabus_id, code, competency_type, description, order_index, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(competency.id)
        .bind(competency.tenant_id)
        .bind(competency.syllabus_id)
        .bind(&competency.code)
        .bind(&competency.competency_type)
        .bind(&competency.description)
        .bind(competency.order_index)
        .bind(competency.created_at)
        .bind(competency.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_competencies(
        &self,
        syllabus_id: Uuid,
    ) -> Result<Vec<SyllabusCompetency>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, syllabus_id, code, competency_type, description, order_index, created_at, updated_at, deleted_at
               FROM syllabus_competencies WHERE syllabus_id = $1 AND deleted_at IS NULL
               ORDER BY order_index ASC"#
        )
        .bind(syllabus_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| SyllabusCompetency {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                syllabus_id: r.get("syllabus_id"),
                code: r.get("code"),
                competency_type: r.get("competency_type"),
                description: r.get("description"),
                order_index: r.get("order_index"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(items)
    }
}
