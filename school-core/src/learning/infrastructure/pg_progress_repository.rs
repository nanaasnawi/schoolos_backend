use crate::common::error::InfrastructureError;
use crate::learning::domain::student_progress::StudentProgress;
use crate::learning::infrastructure::repository_traits::StudentProgressRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn text_to_f64(val: Option<&str>) -> Option<f64> {
    val.and_then(|s| s.parse::<f64>().ok())
}

pub struct PgStudentProgressRepository {
    pool: PgPool,
}

impl PgStudentProgressRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StudentProgressRepository for PgStudentProgressRepository {
    async fn save(&self, progress: &StudentProgress) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO student_progress (id, tenant_id, student_id, class_id, subject_id, overall_progress,
                lesson_completed, lesson_total, assignment_completed, assignment_total,
                quiz_completed, quiz_total, session_attended, session_total,
                calculated_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6::TEXT::NUMERIC, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
            SET overall_progress = $6::TEXT::NUMERIC,
                lesson_completed = $7, lesson_total = $8,
                assignment_completed = $9, assignment_total = $10,
                quiz_completed = $11, quiz_total = $12,
                session_attended = $13, session_total = $14,
                calculated_at = $15, updated_at = $17
            "#
        )
        .bind(progress.id)
        .bind(progress.tenant_id)
        .bind(progress.student_id)
        .bind(progress.class_id)
        .bind(progress.subject_id)
        .bind(progress.overall_progress.to_string())
        .bind(progress.lesson_completed)
        .bind(progress.lesson_total)
        .bind(progress.assignment_completed)
        .bind(progress.assignment_total)
        .bind(progress.quiz_completed)
        .bind(progress.quiz_total)
        .bind(progress.session_attended)
        .bind(progress.session_total)
        .bind(progress.calculated_at)
        .bind(progress.created_at)
        .bind(progress.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_by_student_class_subject(
        &self,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<StudentProgress>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id,
               overall_progress::TEXT AS overall_progress,
               lesson_completed, lesson_total, assignment_completed, assignment_total,
               quiz_completed, quiz_total, session_attended, session_total,
               calculated_at, created_at, updated_at
               FROM student_progress WHERE student_id = $1 AND class_id = $2 AND subject_id = $3"#,
        )
        .bind(student_id)
        .bind(class_id)
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            let op_str: Option<String> = r.get("overall_progress");
            StudentProgress {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                student_id: r.get("student_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                overall_progress: text_to_f64(op_str.as_deref()).unwrap_or(0.0),
                lesson_completed: r.get("lesson_completed"),
                lesson_total: r.get("lesson_total"),
                assignment_completed: r.get("assignment_completed"),
                assignment_total: r.get("assignment_total"),
                quiz_completed: r.get("quiz_completed"),
                quiz_total: r.get("quiz_total"),
                session_attended: r.get("session_attended"),
                session_total: r.get("session_total"),
                calculated_at: r.get("calculated_at"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            }
        }))
    }

    async fn find_by_class(
        &self,
        class_id: Uuid,
    ) -> Result<Vec<StudentProgress>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id,
               overall_progress::TEXT AS overall_progress,
               lesson_completed, lesson_total, assignment_completed, assignment_total,
               quiz_completed, quiz_total, session_attended, session_total,
               calculated_at, created_at, updated_at
               FROM student_progress WHERE class_id = $1
               ORDER BY student_id"#,
        )
        .bind(class_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let op_str: Option<String> = r.get("overall_progress");
                StudentProgress {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    student_id: r.get("student_id"),
                    class_id: r.get("class_id"),
                    subject_id: r.get("subject_id"),
                    overall_progress: text_to_f64(op_str.as_deref()).unwrap_or(0.0),
                    lesson_completed: r.get("lesson_completed"),
                    lesson_total: r.get("lesson_total"),
                    assignment_completed: r.get("assignment_completed"),
                    assignment_total: r.get("assignment_total"),
                    quiz_completed: r.get("quiz_completed"),
                    quiz_total: r.get("quiz_total"),
                    session_attended: r.get("session_attended"),
                    session_total: r.get("session_total"),
                    calculated_at: r.get("calculated_at"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                    domain_events: Vec::new(),
                    version: 1,
                }
            })
            .collect();

        Ok(items)
    }
}
