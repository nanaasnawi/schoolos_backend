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
            INSERT INTO learning_sessions (
                id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
                substitute_teacher_id, session_date, session_number, start_time, end_time,
                scheduled_at, started_at, ended_at, status, notes, cancellation_reason, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
            "#
        )
        .bind(session.id)
        .bind(session.tenant_id)
        .bind(&session.session_type)
        .bind(session.schedule_id)
        .bind(session.lesson_id)
        .bind(session.class_id)
        .bind(session.subject_id)
        .bind(session.teacher_id)
        .bind(session.substitute_teacher_id)
        .bind(session.session_date)
        .bind(session.session_number)
        .bind(session.start_time)
        .bind(session.end_time)
        .bind(session.scheduled_at)
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(&session.status)
        .bind(&session.notes)
        .bind(&session.cancellation_reason)
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
            SET status = $1, started_at = $2, ended_at = $3, notes = $4,
                substitute_teacher_id = $5, cancellation_reason = $6, updated_at = $7
            WHERE id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(&session.status)
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(&session.notes)
        .bind(session.substitute_teacher_id)
        .bind(&session.cancellation_reason)
        .bind(session.updated_at)
        .bind(session.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<LearningSession>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
                      substitute_teacher_id, session_date, session_number, start_time, end_time,
                      scheduled_at, started_at, ended_at, status, notes, cancellation_reason,
                      created_at, updated_at, deleted_at, deleted_by
               FROM learning_sessions WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| LearningSession {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            session_type: r.get::<Option<String>, _>("session_type").unwrap_or_else(|| "scheduled".to_string()),
            schedule_id: r.get("schedule_id"),
            lesson_id: r.get("lesson_id"),
            class_id: r.get("class_id"),
            subject_id: r.get("subject_id"),
            teacher_id: r.get("teacher_id"),
            substitute_teacher_id: r.get("substitute_teacher_id"),
            session_date: r.get::<Option<chrono::NaiveDate>, _>("session_date").unwrap_or_else(|| chrono::Utc::now().date_naive()),
            session_number: r.get::<Option<i32>, _>("session_number").unwrap_or(1),
            start_time: r.get("start_time"),
            end_time: r.get("end_time"),
            scheduled_at: r.get("scheduled_at"),
            started_at: r.get("started_at"),
            ended_at: r.get("ended_at"),
            status: r.get("status"),
            notes: r.get("notes"),
            cancellation_reason: r.get("cancellation_reason"),
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
            r#"SELECT id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
                      substitute_teacher_id, session_date, session_number, start_time, end_time,
                      scheduled_at, started_at, ended_at, status, notes, cancellation_reason,
                      created_at, updated_at, deleted_at, deleted_by
               FROM learning_sessions WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY session_date DESC, created_at DESC"#
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
                session_type: r.get::<Option<String>, _>("session_type").unwrap_or_else(|| "scheduled".to_string()),
                schedule_id: r.get("schedule_id"),
                lesson_id: r.get("lesson_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                teacher_id: r.get("teacher_id"),
                substitute_teacher_id: r.get("substitute_teacher_id"),
                session_date: r.get::<Option<chrono::NaiveDate>, _>("session_date").unwrap_or_else(|| chrono::Utc::now().date_naive()),
                session_number: r.get::<Option<i32>, _>("session_number").unwrap_or(1),
                start_time: r.get("start_time"),
                end_time: r.get("end_time"),
                scheduled_at: r.get("scheduled_at"),
                started_at: r.get("started_at"),
                ended_at: r.get("ended_at"),
                status: r.get("status"),
                notes: r.get("notes"),
                cancellation_reason: r.get("cancellation_reason"),
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
            INSERT INTO session_attendances (id, tenant_id, session_id, student_id, status, checked_in_at, notes, method, recorded_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (session_id, student_id) DO UPDATE
            SET status = EXCLUDED.status,
                checked_in_at = COALESCE(EXCLUDED.checked_in_at, session_attendances.checked_in_at),
                notes = COALESCE(EXCLUDED.notes, session_attendances.notes),
                method = COALESCE(EXCLUDED.method, session_attendances.method),
                recorded_by = COALESCE(EXCLUDED.recorded_by, session_attendances.recorded_by),
                updated_at = EXCLUDED.updated_at
            "#
        )
        .bind(attendance.id)
        .bind(attendance.tenant_id)
        .bind(attendance.session_id)
        .bind(attendance.student_id)
        .bind(&attendance.status)
        .bind(attendance.checked_in_at)
        .bind(&attendance.notes)
        .bind(&attendance.method)
        .bind(attendance.recorded_by)
        .bind(attendance.created_at)
        .bind(attendance.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn record_attendance_bulk(
        &self,
        attendances: &[SessionAttendance],
    ) -> Result<(), InfrastructureError> {
        if attendances.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(InfrastructureError::Database)?;

        for attendance in attendances {
            sqlx::query(
                r#"
                INSERT INTO session_attendances (id, tenant_id, session_id, student_id, status, checked_in_at, notes, method, recorded_by, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (session_id, student_id) DO UPDATE
                SET status = EXCLUDED.status,
                    checked_in_at = COALESCE(EXCLUDED.checked_in_at, session_attendances.checked_in_at),
                    notes = COALESCE(EXCLUDED.notes, session_attendances.notes),
                    method = COALESCE(EXCLUDED.method, session_attendances.method),
                    recorded_by = COALESCE(EXCLUDED.recorded_by, session_attendances.recorded_by),
                    updated_at = EXCLUDED.updated_at
                "#
            )
            .bind(attendance.id)
            .bind(attendance.tenant_id)
            .bind(attendance.session_id)
            .bind(attendance.student_id)
            .bind(&attendance.status)
            .bind(attendance.checked_in_at)
            .bind(&attendance.notes)
            .bind(&attendance.method)
            .bind(attendance.recorded_by)
            .bind(attendance.created_at)
            .bind(attendance.updated_at)
            .execute(&mut *tx)
            .await
            .map_err(InfrastructureError::Database)?;
        }

        tx.commit().await.map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_attendance(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, session_id, student_id, status, checked_in_at, notes, method, recorded_by, created_at, updated_at
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
                method: r.try_get("method").ok(),
                recorded_by: r.try_get("recorded_by").ok(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(items)
    }

    async fn find_attendance_by_tenant_and_session(
        &self,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, session_id, student_id, status, checked_in_at, notes, method, recorded_by, created_at, updated_at
               FROM session_attendances WHERE tenant_id = $1 AND session_id = $2
               ORDER BY checked_in_at ASC NULLS LAST"#
        )
        .bind(tenant_id)
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
                method: r.try_get("method").ok(),
                recorded_by: r.try_get("recorded_by").ok(),
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

    async fn find_attendance_by_class(
        &self,
        tenant_id: Uuid,
        class_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT sa.id, sa.tenant_id, sa.session_id, sa.student_id, sa.status, sa.checked_in_at, sa.notes, sa.method, sa.recorded_by, sa.created_at, sa.updated_at
               FROM session_attendances sa
               LEFT JOIN learning_sessions ls ON ls.id = sa.session_id
               LEFT JOIN class_schedules cs ON cs.id = sa.session_id
               WHERE sa.tenant_id = $1 AND (ls.class_id = $2 OR cs.class_id = $2)
               ORDER BY sa.created_at DESC"#
        )
        .bind(tenant_id)
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
                method: r.try_get("method").ok(),
                recorded_by: r.try_get("recorded_by").ok(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
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
            r#"SELECT sa.id, sa.tenant_id, sa.session_id, sa.student_id, sa.status, sa.checked_in_at, sa.notes, sa.method, sa.recorded_by, sa.created_at, sa.updated_at
               FROM session_attendances sa
               LEFT JOIN learning_sessions ls ON ls.id = sa.session_id
               LEFT JOIN class_schedules cs ON cs.id = sa.session_id
               WHERE sa.student_id = $1 AND (ls.class_id = $2 OR cs.class_id = $2)
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
                method: r.try_get("method").ok(),
                recorded_by: r.try_get("recorded_by").ok(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(items)
    }
}
