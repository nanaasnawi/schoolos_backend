use crate::common::error::InfrastructureError;
use crate::learning::domain::lesson::Lesson;
use crate::learning::domain::lesson_plan::LessonPlan;
use crate::learning::infrastructure::repository_traits::{LessonPlanRepository, LessonRepository};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgLessonRepository {
    pool: PgPool,
}

impl PgLessonRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LessonRepository for PgLessonRepository {
    async fn create(&self, lesson: &Lesson) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO lessons (id, tenant_id, syllabus_id, code, title, description, learning_objectives, duration_minutes, order_index, status, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(lesson.id)
        .bind(lesson.tenant_id)
        .bind(lesson.syllabus_id)
        .bind(&lesson.code)
        .bind(&lesson.title)
        .bind(&lesson.description)
        .bind(&lesson.learning_objectives)
        .bind(lesson.duration_minutes)
        .bind(lesson.order_index)
        .bind(&lesson.status)
        .bind(lesson.is_active)
        .bind(lesson.created_at)
        .bind(lesson.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Lesson>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, syllabus_id, code, title, description, learning_objectives, duration_minutes, order_index, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM lessons WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Lesson {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            syllabus_id: r.get("syllabus_id"),
            code: r.get("code"),
            title: r.get("title"),
            description: r.get("description"),
            learning_objectives: r.get("learning_objectives"),
            duration_minutes: r.get("duration_minutes"),
            order_index: r.get("order_index"),
            status: r.get("status"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Lesson>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, syllabus_id, code, title, description, learning_objectives, duration_minutes, order_index, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM lessons WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY order_index ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Lesson {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                syllabus_id: r.get("syllabus_id"),
                code: r.get("code"),
                title: r.get("title"),
                description: r.get("description"),
                learning_objectives: r.get("learning_objectives"),
                duration_minutes: r.get("duration_minutes"),
                order_index: r.get("order_index"),
                status: r.get("status"),
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

    async fn update(&self, lesson: &Lesson) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE lessons
            SET title = $1, description = $2, learning_objectives = $3, duration_minutes = $4, status = $5, updated_at = NOW()
            WHERE id = $6 AND tenant_id = $7 AND deleted_at IS NULL
            "#
        )
        .bind(&lesson.title)
        .bind(&lesson.description)
        .bind(&lesson.learning_objectives)
        .bind(lesson.duration_minutes)
        .bind(&lesson.status)
        .bind(lesson.id)
        .bind(lesson.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE lessons
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

#[async_trait]
impl LessonPlanRepository for PgLessonRepository {
    async fn create(&self, plan: &LessonPlan) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO lesson_plans (id, tenant_id, lesson_id, teaching_methods, activities_opening, activities_core, activities_closing, resources, assessment_criteria, version, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(plan.id)
        .bind(plan.tenant_id)
        .bind(plan.lesson_id)
        .bind(&plan.teaching_methods)
        .bind(&plan.activities_opening)
        .bind(&plan.activities_core)
        .bind(&plan.activities_closing)
        .bind(&plan.resources)
        .bind(&plan.assessment_criteria)
        .bind(plan.version)
        .bind(plan.created_at)
        .bind(plan.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_lesson_id(
        &self,
        lesson_id: Uuid,
    ) -> Result<Option<LessonPlan>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, teaching_methods, activities_opening, activities_core, activities_closing, resources, assessment_criteria, version, created_at, updated_at, deleted_at
               FROM lesson_plans WHERE lesson_id = $1 AND deleted_at IS NULL LIMIT 1"#
        )
        .bind(lesson_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| LessonPlan {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            teaching_methods: r.get("teaching_methods"),
            activities_opening: r.get("activities_opening"),
            activities_core: r.get("activities_core"),
            activities_closing: r.get("activities_closing"),
            resources: r.get("resources"),
            assessment_criteria: r.get("assessment_criteria"),
            version: r.get("version"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            domain_events: Vec::new(),
        }))
    }
}
