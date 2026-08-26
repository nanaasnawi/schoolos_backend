use crate::common::error::InfrastructureError;
use crate::learning::domain::assignment::Assignment;
use crate::learning::domain::assignment_submission::AssignmentSubmission;
use crate::learning::infrastructure::repository_traits::AssignmentRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgAssignmentRepository {
    pool: PgPool,
}

impl PgAssignmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AssignmentRepository for PgAssignmentRepository {
    async fn create(&self, assignment: &Assignment) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO assignments (id, tenant_id, lesson_id, title, description, instructions, max_score, due_at, assignment_type, status, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(assignment.id)
        .bind(assignment.tenant_id)
        .bind(assignment.lesson_id)
        .bind(&assignment.title)
        .bind(&assignment.description)
        .bind(&assignment.instructions)
        .bind(assignment.max_score)
        .bind(assignment.due_at)
        .bind(&assignment.assignment_type)
        .bind(&assignment.status)
        .bind(assignment.is_active)
        .bind(assignment.created_at)
        .bind(assignment.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Assignment>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, instructions, max_score, due_at, assignment_type, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assignments WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Assignment {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            title: r.get("title"),
            description: r.get("description"),
            instructions: r.get("instructions"),
            max_score: r.get("max_score"),
            due_at: r.get("due_at"),
            assignment_type: r.get("assignment_type"),
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

    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<Assignment>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, instructions, max_score, due_at, assignment_type, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assignments WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Assignment {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
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

    async fn update(&self, assignment: &Assignment) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE assignments
            SET title = $1, description = $2, instructions = $3, max_score = $4, due_at = $5, status = $6, updated_at = NOW()
            WHERE id = $7 AND tenant_id = $8 AND deleted_at IS NULL
            "#
        )
        .bind(&assignment.title)
        .bind(&assignment.description)
        .bind(&assignment.instructions)
        .bind(assignment.max_score)
        .bind(assignment.due_at)
        .bind(&assignment.status)
        .bind(assignment.id)
        .bind(assignment.tenant_id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE assignments
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

    async fn find_by_lesson(
        &self,
        lesson_id: Uuid,
    ) -> Result<Vec<Assignment>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, instructions, max_score, due_at, assignment_type, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assignments WHERE lesson_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(lesson_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Assignment {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
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

    async fn list_published(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<Assignment>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, instructions, max_score, due_at, assignment_type, status, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assignments WHERE tenant_id = $1 AND status = 'published' AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| Assignment {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
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

    async fn count_by_lesson(&self, lesson_id: Uuid) -> Result<i64, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT COUNT(*) as count FROM assignments WHERE lesson_id = $1 AND deleted_at IS NULL"#
        )
        .bind(lesson_id)
        .fetch_one(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let count: i64 = record.get("count");
        Ok(count)
    }

    async fn submit(&self, submission: &AssignmentSubmission) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO assignment_submissions (id, tenant_id, assignment_id, student_id, content, file_url, submitted_at, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (assignment_id, student_id) DO UPDATE
            SET content = COALESCE($5, assignment_submissions.content),
                file_url = COALESCE($6, assignment_submissions.file_url),
                status = 'submitted',
                submitted_at = $7,
                updated_at = $10
            "#
        )
        .bind(submission.id)
        .bind(submission.tenant_id)
        .bind(submission.assignment_id)
        .bind(submission.student_id)
        .bind(&submission.content)
        .bind(&submission.file_url)
        .bind(submission.submitted_at)
        .bind(&submission.status)
        .bind(submission.created_at)
        .bind(submission.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn update_submission(
        &self,
        submission: &AssignmentSubmission,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE assignment_submissions
            SET score = $1, feedback = $2, graded_at = $3, graded_by = $4, status = $5, updated_at = $6
            WHERE id = $7
            "#
        )
        .bind(submission.score)
        .bind(&submission.feedback)
        .bind(submission.graded_at)
        .bind(submission.graded_by)
        .bind(&submission.status)
        .bind(submission.updated_at)
        .bind(submission.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_submission_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AssignmentSubmission>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, assignment_id, student_id, content, file_url, submitted_at, status, score, feedback, graded_at, graded_by, created_at, updated_at
               FROM assignment_submissions WHERE id = $1"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| AssignmentSubmission {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            assignment_id: r.get("assignment_id"),
            student_id: r.get("student_id"),
            content: r.get("content"),
            file_url: r.get("file_url"),
            submitted_at: r.get("submitted_at"),
            status: r.get("status"),
            score: r.get("score"),
            feedback: r.get("feedback"),
            graded_at: r.get("graded_at"),
            graded_by: r.get("graded_by"),
            attempts: Vec::new(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn add_attempt(
        &self,
        attempt: &crate::learning::domain::assignment_submission::SubmissionAttempt,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO submission_attempts (id, tenant_id, submission_id, attempt_number, content, file_url, checksum, submitted_at, is_late, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            "#
        )
        .bind(attempt.id)
        .bind(Uuid::nil()) // Fallback tenant ID if not on attempt
        .bind(attempt.submission_id)
        .bind(attempt.attempt_number)
        .bind(&attempt.content)
        .bind(&attempt.file_url)
        .bind(&attempt.checksum)
        .bind(attempt.submitted_at)
        .bind(attempt.is_late)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_attempts(
        &self,
        submission_id: Uuid,
    ) -> Result<
        Vec<crate::learning::domain::assignment_submission::SubmissionAttempt>,
        InfrastructureError,
    > {
        let records = sqlx::query(
            r#"SELECT id, submission_id, attempt_number, content, file_url, checksum, submitted_at, is_late
               FROM submission_attempts WHERE submission_id = $1
               ORDER BY attempt_number ASC"#
        )
        .bind(submission_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(
                |r| crate::learning::domain::assignment_submission::SubmissionAttempt {
                    id: r.get("id"),
                    submission_id: r.get("submission_id"),
                    attempt_number: r.get("attempt_number"),
                    content: r.get("content"),
                    file_url: r.get("file_url"),
                    checksum: r.get("checksum"),
                    submitted_at: r.get("submitted_at"),
                    is_late: r.get("is_late"),
                },
            )
            .collect();

        Ok(items)
    }

    async fn list_pending_grading(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<AssignmentSubmission>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, assignment_id, student_id, content, file_url, submitted_at, status, score, feedback, graded_at, graded_by, created_at, updated_at
               FROM assignment_submissions WHERE tenant_id = $1 AND status IN ('submitted', 'grading', 'late')
               ORDER BY submitted_at ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| AssignmentSubmission {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                assignment_id: r.get("assignment_id"),
                student_id: r.get("student_id"),
                content: r.get("content"),
                file_url: r.get("file_url"),
                submitted_at: r.get("submitted_at"),
                status: r.get("status"),
                score: r.get("score"),
                feedback: r.get("feedback"),
                graded_at: r.get("graded_at"),
                graded_by: r.get("graded_by"),
                attempts: Vec::new(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(items)
    }

    async fn find_submissions(
        &self,
        assignment_id: Uuid,
    ) -> Result<Vec<AssignmentSubmission>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, assignment_id, student_id, content, file_url, submitted_at, status, score, feedback, graded_at, graded_by, created_at, updated_at
               FROM assignment_submissions WHERE assignment_id = $1
               ORDER BY submitted_at DESC"#
        )
        .bind(assignment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| AssignmentSubmission {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                assignment_id: r.get("assignment_id"),
                student_id: r.get("student_id"),
                content: r.get("content"),
                file_url: r.get("file_url"),
                submitted_at: r.get("submitted_at"),
                status: r.get("status"),
                score: r.get("score"),
                feedback: r.get("feedback"),
                graded_at: r.get("graded_at"),
                graded_by: r.get("graded_by"),
                attempts: Vec::new(),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(items)
    }
}
