use crate::common::error::InfrastructureError;
use crate::learning::domain::attempt_answer::AttemptAnswer;
use crate::learning::domain::quiz::Quiz;
use crate::learning::domain::quiz_attempt::QuizAttempt;
use crate::learning::domain::quiz_choice::QuizChoice;
use crate::learning::domain::quiz_question::QuizQuestion;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgQuizRepository {
    pool: PgPool,
}

impl PgQuizRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl QuizRepository for PgQuizRepository {
    async fn create(&self, quiz: &Quiz) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO quizzes (id, tenant_id, lesson_id, title, description, time_limit_minutes, passing_score, max_score, max_attempts, shuffle_questions, shuffle_choices, start_at, end_at, status, questions_count, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#
        )
        .bind(quiz.id)
        .bind(quiz.tenant_id)
        .bind(quiz.lesson_id)
        .bind(&quiz.title)
        .bind(&quiz.description)
        .bind(quiz.duration_minutes)
        .bind(quiz.passing_score)
        .bind(quiz.max_score)
        .bind(quiz.max_attempts)
        .bind(quiz.shuffle_questions)
        .bind(quiz.shuffle_choices)
        .bind(quiz.start_at)
        .bind(quiz.end_at)
        .bind(&quiz.status)
        .bind(quiz.questions_count)
        .bind(quiz.is_active)
        .bind(quiz.created_at)
        .bind(quiz.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn update(&self, quiz: &Quiz) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE quizzes SET title = $1, description = $2, time_limit_minutes = $3, passing_score = $4,
            max_score = $5, max_attempts = $6, shuffle_questions = $7, shuffle_choices = $8, start_at = $9, end_at = $10,
            status = $11, questions_count = $12, is_active = $13, updated_at = $14
            WHERE id = $15 AND deleted_at IS NULL
            "#
        )
        .bind(&quiz.title)
        .bind(&quiz.description)
        .bind(quiz.duration_minutes)
        .bind(quiz.passing_score)
        .bind(quiz.max_score)
        .bind(quiz.max_attempts)
        .bind(quiz.shuffle_questions)
        .bind(quiz.shuffle_choices)
        .bind(quiz.start_at)
        .bind(quiz.end_at)
        .bind(&quiz.status)
        .bind(quiz.questions_count)
        .bind(quiz.is_active)
        .bind(quiz.updated_at)
        .bind(quiz.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"UPDATE quizzes SET deleted_at = NOW(), deleted_by = $1, is_active = false WHERE id = $2 AND deleted_at IS NULL"#
        )
        .bind(deleted_by)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Quiz>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, time_limit_minutes, passing_score, max_score, max_attempts, shuffle_questions, shuffle_choices, start_at, end_at, status, questions_count, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM quizzes WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let questions = self.find_questions(id).await?;
            Ok(Some(Quiz {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                duration_minutes: r.get::<Option<i32>, _>("time_limit_minutes").unwrap_or(30),
                passing_score: r.get("passing_score"),
                max_score: r.get("max_score"),
                max_attempts: r.get::<Option<i32>, _>("max_attempts").unwrap_or(1),
                shuffle_questions: r
                    .get::<Option<bool>, _>("shuffle_questions")
                    .unwrap_or(false),
                shuffle_choices: r.get::<Option<bool>, _>("shuffle_choices").unwrap_or(false),
                start_at: r.get("start_at"),
                end_at: r.get("end_at"),
                status: r.get("status"),
                questions_count: r.get("questions_count"),
                questions,
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Quiz>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, lesson_id, title, description, time_limit_minutes, passing_score, max_score, max_attempts, shuffle_questions, shuffle_choices, start_at, end_at, status, questions_count, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM quizzes WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut items = Vec::new();
        for r in records {
            let quiz_id: Uuid = r.get("id");
            let questions = self.find_questions(quiz_id).await?;
            items.push(Quiz {
                id: quiz_id,
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                duration_minutes: r.get::<Option<i32>, _>("time_limit_minutes").unwrap_or(30),
                passing_score: r.get("passing_score"),
                max_score: r.get("max_score"),
                max_attempts: r.get::<Option<i32>, _>("max_attempts").unwrap_or(1),
                shuffle_questions: r
                    .get::<Option<bool>, _>("shuffle_questions")
                    .unwrap_or(false),
                shuffle_choices: r.get::<Option<bool>, _>("shuffle_choices").unwrap_or(false),
                start_at: r.get("start_at"),
                end_at: r.get("end_at"),
                status: r.get("status"),
                questions_count: r.get("questions_count"),
                questions,
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            });
        }

        Ok(items)
    }

    async fn save_question(&self, question: &QuizQuestion) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO quiz_questions (id, quiz_id, question_text, question_type, points, order_index, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(question.id)
        .bind(question.quiz_id)
        .bind(&question.question_text)
        .bind(&question.question_type)
        .bind(question.points)
        .bind(question.order_index)
        .bind(question.created_at)
        .bind(question.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        for choice in &question.choices {
            self.save_choice(choice).await?;
        }

        Ok(())
    }

    async fn save_choice(&self, choice: &QuizChoice) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO quiz_choices (id, question_id, choice_text, is_correct, order_index, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(choice.id)
        .bind(choice.question_id)
        .bind(&choice.choice_text)
        .bind(choice.is_correct)
        .bind(choice.order_index)
        .bind(choice.created_at)
        .bind(choice.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_questions(
        &self,
        quiz_id: Uuid,
    ) -> Result<Vec<QuizQuestion>, InfrastructureError> {
        let question_rows = sqlx::query(
            r#"SELECT id, quiz_id, question_text, question_type, points, order_index, created_at, updated_at
               FROM quiz_questions WHERE quiz_id = $1
               ORDER BY order_index ASC"#
        )
        .bind(quiz_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut questions = Vec::new();
        for qr in question_rows {
            let question_id: Uuid = qr.get("id");
            let choice_rows = sqlx::query(
                r#"SELECT id, question_id, choice_text, is_correct, order_index, created_at, updated_at
                   FROM quiz_choices WHERE question_id = $1
                   ORDER BY order_index ASC"#
            )
            .bind(question_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let choices = choice_rows
                .into_iter()
                .map(|cr| QuizChoice {
                    id: cr.get("id"),
                    question_id: cr.get("question_id"),
                    choice_text: cr.get("choice_text"),
                    is_correct: cr.get("is_correct"),
                    order_index: cr.get("order_index"),
                    created_at: cr.get("created_at"),
                    updated_at: cr.get("updated_at"),
                })
                .collect();

            questions.push(QuizQuestion {
                id: question_id,
                quiz_id: qr.get("quiz_id"),
                question_text: qr.get("question_text"),
                question_type: qr.get("question_type"),
                points: qr.get("points"),
                order_index: qr.get("order_index"),
                choices,
                created_at: qr.get("created_at"),
                updated_at: qr.get("updated_at"),
            });
        }

        Ok(questions)
    }

    async fn create_attempt(&self, attempt: &QuizAttempt) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO quiz_attempts (id, tenant_id, quiz_id, student_id, attempt_number, started_at, completed_at, score, total_points, passed, status, shuffle_seed, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#
        )
        .bind(attempt.id)
        .bind(attempt.tenant_id)
        .bind(attempt.quiz_id)
        .bind(attempt.student_id)
        .bind(attempt.attempt_number)
        .bind(attempt.started_at)
        .bind(attempt.completed_at)
        .bind(attempt.score)
        .bind(attempt.total_points)
        .bind(attempt.passed)
        .bind(&attempt.status)
        .bind(attempt.shuffle_seed)
        .bind(attempt.created_at)
        .bind(attempt.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn update_attempt(&self, attempt: &QuizAttempt) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE quiz_attempts
            SET completed_at = $1, score = $2, passed = $3, status = $4, updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(attempt.completed_at)
        .bind(attempt.score)
        .bind(attempt.passed)
        .bind(&attempt.status)
        .bind(attempt.updated_at)
        .bind(attempt.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_attempt_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<QuizAttempt>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, quiz_id, student_id, attempt_number, started_at, completed_at, score, total_points, passed, status, shuffle_seed, created_at, updated_at
               FROM quiz_attempts WHERE id = $1"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let attempt_id: Uuid = r.get("id");
            let answers = self.find_answers(attempt_id).await?;

            Ok(Some(QuizAttempt {
                id: attempt_id,
                tenant_id: r.get("tenant_id"),
                quiz_id: r.get("quiz_id"),
                student_id: r.get("student_id"),
                attempt_number: r.get::<Option<i32>, _>("attempt_number").unwrap_or(1),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                score: r.get("score"),
                total_points: r.get("total_points"),
                passed: r.get::<Option<bool>, _>("passed").unwrap_or(false),
                status: r.get("status"),
                shuffle_seed: r.get::<Option<i64>, _>("shuffle_seed").unwrap_or(0),
                answers,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_attempts_by_quiz(
        &self,
        quiz_id: Uuid,
    ) -> Result<Vec<QuizAttempt>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, quiz_id, student_id, attempt_number, started_at, completed_at, score, total_points, passed, status, shuffle_seed, created_at, updated_at
               FROM quiz_attempts WHERE quiz_id = $1
               ORDER BY started_at DESC"#
        )
        .bind(quiz_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut attempts = Vec::new();
        for r in records {
            let attempt_id: Uuid = r.get("id");
            let answers = self.find_answers(attempt_id).await?;

            attempts.push(QuizAttempt {
                id: attempt_id,
                tenant_id: r.get("tenant_id"),
                quiz_id: r.get("quiz_id"),
                student_id: r.get("student_id"),
                attempt_number: r.get::<Option<i32>, _>("attempt_number").unwrap_or(1),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                score: r.get("score"),
                total_points: r.get("total_points"),
                passed: r.get::<Option<bool>, _>("passed").unwrap_or(false),
                status: r.get("status"),
                shuffle_seed: r.get::<Option<i64>, _>("shuffle_seed").unwrap_or(0),
                answers,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            });
        }

        Ok(attempts)
    }

    async fn save_answer(&self, answer: &AttemptAnswer) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO attempt_answers (id, attempt_id, question_id, chosen_choice_id, text_answer, is_correct, points_earned, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (attempt_id, question_id) DO UPDATE
            SET chosen_choice_id = COALESCE($4, attempt_answers.chosen_choice_id),
                text_answer = COALESCE($5, attempt_answers.text_answer),
                is_correct = COALESCE($6, attempt_answers.is_correct),
                points_earned = COALESCE($7, attempt_answers.points_earned)
            "#
        )
        .bind(answer.id)
        .bind(answer.attempt_id)
        .bind(answer.question_id)
        .bind(answer.chosen_choice_id)
        .bind(&answer.text_answer)
        .bind(answer.is_correct)
        .bind(answer.points_earned)
        .bind(answer.created_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_attempts_by_student(
        &self,
        student_id: Uuid,
        class_id: Uuid,
    ) -> Result<Vec<QuizAttempt>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT qa.id, qa.tenant_id, qa.quiz_id, qa.student_id, qa.attempt_number, qa.started_at, qa.completed_at, qa.score, qa.total_points, qa.passed, qa.status, qa.shuffle_seed, qa.created_at, qa.updated_at
               FROM quiz_attempts qa
               JOIN quizzes q ON q.id = qa.quiz_id
               WHERE qa.student_id = $1 AND q.lesson_id IN (
                   SELECT id FROM lessons WHERE class_id = $2
               )
               ORDER BY qa.started_at DESC"#
        )
        .bind(student_id)
        .bind(class_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut attempts = Vec::new();
        for r in records {
            let attempt_id: Uuid = r.get("id");
            let answers = self.find_answers(attempt_id).await?;

            attempts.push(QuizAttempt {
                id: attempt_id,
                tenant_id: r.get("tenant_id"),
                quiz_id: r.get("quiz_id"),
                student_id: r.get("student_id"),
                attempt_number: r.get::<Option<i32>, _>("attempt_number").unwrap_or(1),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                score: r.get("score"),
                total_points: r.get("total_points"),
                passed: r.get::<Option<bool>, _>("passed").unwrap_or(false),
                status: r.get("status"),
                shuffle_seed: r.get::<Option<i64>, _>("shuffle_seed").unwrap_or(0),
                answers,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            });
        }

        Ok(attempts)
    }

    async fn find_answers(
        &self,
        attempt_id: Uuid,
    ) -> Result<Vec<AttemptAnswer>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, attempt_id, question_id, chosen_choice_id, text_answer, is_correct, points_earned, created_at
               FROM attempt_answers WHERE attempt_id = $1"#
        )
        .bind(attempt_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| AttemptAnswer {
                id: r.get("id"),
                attempt_id: r.get("attempt_id"),
                question_id: r.get("question_id"),
                chosen_choice_id: r.get("chosen_choice_id"),
                text_answer: r.get("text_answer"),
                is_correct: r.get("is_correct"),
                points_earned: r.get("points_earned"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(items)
    }
}
