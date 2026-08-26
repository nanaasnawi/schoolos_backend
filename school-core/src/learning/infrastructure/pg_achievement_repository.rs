use crate::common::error::InfrastructureError;
use crate::learning::domain::achievement::{Achievement, StudentAchievement};
use crate::learning::infrastructure::repository_traits::AchievementRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgAchievementRepository {
    pool: PgPool,
}

impl PgAchievementRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AchievementRepository for PgAchievementRepository {
    async fn save(&self, achievement: &Achievement) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO achievements (id, tenant_id, title, description, icon, criteria_type, criteria_value, is_published, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE
            SET title = $3, description = $4, icon = $5, criteria_type = $6, criteria_value = $7,
                is_published = $8, updated_at = $10
            "#
        )
        .bind(achievement.id)
        .bind(achievement.tenant_id)
        .bind(&achievement.title)
        .bind(&achievement.description)
        .bind(&achievement.icon)
        .bind(&achievement.criteria_type)
        .bind(&achievement.criteria_value)
        .bind(achievement.is_published)
        .bind(achievement.created_at)
        .bind(achievement.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Achievement>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, title, description, icon, criteria_type, criteria_value,
               is_published, created_at, updated_at, deleted_at, deleted_by
               FROM achievements WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Achievement {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            title: r.get("title"),
            description: r.get("description"),
            icon: r.get("icon"),
            criteria_type: r.get("criteria_type"),
            criteria_value: r.get("criteria_value"),
            is_published: r.get("is_published"),
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
    ) -> Result<Vec<Achievement>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, title, description, icon, criteria_type, criteria_value,
               is_published, created_at, updated_at, deleted_at, deleted_by
               FROM achievements WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY title ASC"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Achievement {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                title: r.get("title"),
                description: r.get("description"),
                icon: r.get("icon"),
                criteria_type: r.get("criteria_type"),
                criteria_value: r.get("criteria_value"),
                is_published: r.get("is_published"),
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

    async fn delete(&self, id: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query("UPDATE achievements SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn award(&self, sa: &StudentAchievement) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"INSERT INTO student_achievements (id, tenant_id, student_id, achievement_id, earned_at, created_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (id) DO NOTHING"#
        )
        .bind(sa.id)
        .bind(sa.tenant_id)
        .bind(sa.student_id)
        .bind(sa.achievement_id)
        .bind(sa.earned_at)
        .bind(sa.earned_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_student_achievements(
        &self,
        student_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<StudentAchievement>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, student_id, achievement_id, earned_at
               FROM student_achievements WHERE student_id = $1 AND tenant_id = $2
               ORDER BY earned_at DESC"#,
        )
        .bind(student_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| StudentAchievement {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                student_id: r.get("student_id"),
                achievement_id: r.get("achievement_id"),
                earned_at: r.get("earned_at"),
            })
            .collect();

        Ok(items)
    }

    async fn find_by_student_and_achievement(
        &self,
        student_id: Uuid,
        achievement_id: Uuid,
    ) -> Result<Option<StudentAchievement>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, student_id, achievement_id, earned_at
               FROM student_achievements WHERE student_id = $1 AND achievement_id = $2"#,
        )
        .bind(student_id)
        .bind(achievement_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| StudentAchievement {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            student_id: r.get("student_id"),
            achievement_id: r.get("achievement_id"),
            earned_at: r.get("earned_at"),
        }))
    }
}
