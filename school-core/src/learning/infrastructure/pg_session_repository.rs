use crate::common::error::InfrastructureError;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::domain::session_attendance::SessionAttendance;
use crate::learning::infrastructure::repository_traits::SessionRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgSessionRepository {
    pool: PgPool,
}

impl PgSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PgSessionRepository {
    async fn create(&self, session: &LearningSession) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO learning_sessions (id, tenant_id, lesson_id, class_id, teacher_id, scheduled_at, started_at, ended_at, status, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(session.id)
        .bind(session.tenant_id)
        .bind(session.lesson_id)
        .bind(session.class_id)
        .bind(session.teacher_id)
        .bind(session.scheduled_at)
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(&session.status)
        .bind(&session.notes)
        .bind(session.created_at)
        .bind(session.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn update(&self, session: &LearningSession) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE learning_sessions
            SET status = $1, started_at = $2, ended_at = $3, notes = $4, updated_at = $5
            WHERE id = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(&session.status)
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(&session.notes)
        .bind(session.updated_at)
        .bind(session.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<LearningSession>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, class_id, teacher_id, scheduled_at, started_at, ended_at, status, notes, created_at, updated_at, deleted_at, deleted_by
               FROM learning_sessions WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| LearningSession {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            class_id: r.get("class_id"),
            teacher_id: r.get("teacher_id"),
            scheduled_at: r.get("scheduled_at"),
            started_at: r.get("started_at"),
            ended_at: r.get("ended_at"),
            status: r.get("status"),
            notes: r.get("notes"),
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
    ) -> Result<Vec<LearningSession>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, class_id, teacher_id, scheduled_at, started_at, ended_at, status, notes, created_at, updated_at, deleted_at, deleted_by
               FROM learning_sessions WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| LearningSession {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                class_id: r.get("class_id"),
                teacher_id: r.get("teacher_id"),
                scheduled_at: r.get("scheduled_at"),
                started_at: r.get("started_at"),
                ended_at: r.get("ended_at"),
                status: r.get("status"),
                notes: r.get("notes"),
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

    async fn record_attendance(
        &self,
        attendance: &SessionAttendance,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO session_attendances (id, tenant_id, session_id, student_id, status, checked_in_at, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (session_id, student_id) DO UPDATE
            SET status = $5, checked_in_at = COALESCE($6, session_attendances.checked_in_at), notes = COALESCE($7, session_attendances.notes), updated_at = $9
            "#
        )
        .bind(attendance.id)
        .bind(attendance.tenant_id)
        .bind(attendance.session_id)
        .bind(attendance.student_id)
        .bind(&attendance.status)
        .bind(attendance.checked_in_at)
        .bind(&attendance.notes)
        .bind(attendance.created_at)
        .bind(attendance.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_attendance(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, session_id, student_id, status, checked_in_at, notes, created_at, updated_at
               FROM session_attendances WHERE session_id = $1
               ORDER BY checked_in_at ASC NULLS LAST"#
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| SessionAttendance {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                session_id: r.get("session_id"),
                student_id: r.get("student_id"),
                status: r.get("status"),
                checked_in_at: r.get("checked_in_at"),
                notes: r.get("notes"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(items)
    }

    async fn find_by_class(
        &self,
        class_id: Uuid,
    ) -> Result<Vec<LearningSession>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, class_id, teacher_id, scheduled_at, started_at, ended_at, status, notes, created_at, updated_at, deleted_at, deleted_by
               FROM learning_sessions WHERE class_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(class_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| LearningSession {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                class_id: r.get("class_id"),
                teacher_id: r.get("teacher_id"),
                scheduled_at: r.get("scheduled_at"),
                started_at: r.get("started_at"),
                ended_at: r.get("ended_at"),
                status: r.get("status"),
                notes: r.get("notes"),
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

    async fn find_attendance_by_student(
        &self,
        student_id: Uuid,
        class_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT sa.id, sa.tenant_id, sa.session_id, sa.student_id, sa.status, sa.checked_in_at, sa.notes, sa.created_at, sa.updated_at
               FROM session_attendances sa
               JOIN learning_sessions ls ON ls.id = sa.session_id
               WHERE sa.student_id = $1 AND ls.class_id = $2
               ORDER BY sa.checked_in_at ASC NULLS LAST"#
        )
        .bind(student_id)
        .bind(class_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| SessionAttendance {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                session_id: r.get("session_id"),
                student_id: r.get("student_id"),
                status: r.get("status"),
                checked_in_at: r.get("checked_in_at"),
                notes: r.get("notes"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(items)
    }
}
