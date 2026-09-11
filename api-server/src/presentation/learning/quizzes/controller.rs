use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    attempt_response::AttemptResponse, create_quiz_request::CreateQuizRequest,
    quiz_response::QuizResponse, start_attempt_request::StartAttemptRequest,
    submit_attempt_request::SubmitAttemptRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::quiz::{
    create_quiz::CreateQuizCommand, get_quiz::GetQuizQuery, grade_attempt::GradeAttemptCommand,
    publish_quiz::PublishQuizCommand,
    start_attempt::StartAttemptCommand, submit_attempt::SubmitAnswer,
    submit_attempt::SubmitAttemptCommand,
};

pub fn quiz_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
        .route("/{id}/publish", post(publish))
        .route("/{id}/questions", post(add_question).get(get_questions))
        .route("/{id}/attempts", post(start_attempt))
        .route("/{id}/attempts/{attempt_id}/submit", post(submit_attempt))
        .route("/{id}/attempts/{attempt_id}/grade", post(grade_attempt))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateQuizRequest>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateQuizCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id.unwrap_or_else(Uuid::new_v4),
        title: payload.title,
        description: payload.description.clone(),
        duration_minutes: payload.duration_minutes.unwrap_or(30),
        passing_score: payload.passing_score,
        max_attempts: payload.max_attempts,
        shuffle_questions: payload.shuffle_questions,
        shuffle_choices: payload.shuffle_choices,
        start_at: payload.start_at,
        end_at: payload.end_at,
    };

    let quiz = ctx
        .create_quiz
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar!(r#"SELECT id FROM teachers WHERE user_id = $1 LIMIT 1"#, aid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // Resolve class_id from UUID or class name string (e.g. "PAKET C10")
    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar!(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                    req_ctx.tenant_id,
                    cid_str.trim()
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
        _ => None,
    };

    // Resolve subject_id from description first part (e.g. "Ilmu Pengetahuan Alam dan Sosial (IPAS) • ...")
    let subject_id: Option<Uuid> = if let Some(ref desc) = payload.description {
        let first_part = desc.split('•').next().map(|s| s.trim()).unwrap_or("");
        if !first_part.is_empty() {
            sqlx::query_scalar!(
                r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                req_ctx.tenant_id,
                first_part
            )
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let _ = sqlx::query!(
        r#"UPDATE quizzes SET class_id = $1, subject_id = $2, teacher_id = $3, created_by = $4 WHERE id = $5"#,
        target_class_id,
        subject_id,
        teacher_id,
        actor_id,
        quiz.id
    )
    .execute(&ctx.pool)
    .await;

    let mut resp = QuizResponse::from(quiz);
    resp.class_id = target_class_id;
    if let Some(cid) = target_class_id {
        resp.class_name = sqlx::query_scalar!(r#"SELECT name FROM classes WHERE id = $1"#, cid)
            .fetch_optional(&ctx.pool).await.ok().flatten();
    }
    if let Some(sid) = subject_id {
        resp.subject_name = sqlx::query_scalar!(r#"SELECT name FROM subjects WHERE id = $1"#, sid)
            .fetch_optional(&ctx.pool).await.ok().flatten();
    }
    if let Some(tid) = teacher_id {
        resp.teacher_name = sqlx::query_scalar!(r#"SELECT full_name FROM teachers WHERE id = $1"#, tid)
            .fetch_optional(&ctx.pool).await.ok().flatten();
    }

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<QuizResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru")).unwrap_or(false);
    let is_student = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Siswa")).unwrap_or(false);

    let items: Vec<QuizResponse> = if is_teacher {
        let rows = sqlx::query!(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, q.status, q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 
              AND q.deleted_at IS NULL
              AND (
                  q.created_by = $2 
                  OR q.teacher_id IN (SELECT id FROM teachers WHERE user_id = $2)
                  OR q.teacher_id IS NULL
              )
            ORDER BY q.created_at DESC
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| QuizResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id.unwrap_or_default(),
            title: r.title,
            description: r.description,
            duration_minutes: r.time_limit_minutes.unwrap_or(30),
            passing_score: r.passing_score,
            max_score: r.max_score,
            max_attempts: r.max_attempts,
            shuffle_questions: r.shuffle_questions,
            shuffle_choices: r.shuffle_choices,
            start_at: r.start_at,
            end_at: r.end_at,
            status: r.status,
            questions_count: r.questions_count,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            class_id: r.class_id,
            class_name: r.class_name,
            subject_name: r.subject_name,
            teacher_name: r.teacher_name,
        }).collect()
    } else if is_student {
        let rows = sqlx::query!(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, q.status, q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 
              AND q.deleted_at IS NULL
              AND (
                  q.class_id IS NULL
                  OR q.class_id IN (
                      SELECT en.class_id 
                      FROM students s
                      JOIN enrollments en ON en.student_id = s.id
                      WHERE s.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                  )
              )
            ORDER BY q.created_at DESC
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| QuizResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id.unwrap_or_default(),
            title: r.title,
            description: r.description,
            duration_minutes: r.time_limit_minutes.unwrap_or(30),
            passing_score: r.passing_score,
            max_score: r.max_score,
            max_attempts: r.max_attempts,
            shuffle_questions: r.shuffle_questions,
            shuffle_choices: r.shuffle_choices,
            start_at: r.start_at,
            end_at: r.end_at,
            status: r.status,
            questions_count: r.questions_count,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            class_id: r.class_id,
            class_name: r.class_name,
            subject_name: r.subject_name,
            teacher_name: r.teacher_name,
        }).collect()
    } else {
        let rows = sqlx::query!(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, q.status, q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 AND q.deleted_at IS NULL
            ORDER BY q.created_at DESC
            "#,
            req_ctx.tenant_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| QuizResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id.unwrap_or_default(),
            title: r.title,
            description: r.description,
            duration_minutes: r.time_limit_minutes.unwrap_or(30),
            passing_score: r.passing_score,
            max_score: r.max_score,
            max_attempts: r.max_attempts,
            shuffle_questions: r.shuffle_questions,
            shuffle_choices: r.shuffle_choices,
            start_at: r.start_at,
            end_at: r.end_at,
            status: r.status,
            questions_count: r.questions_count,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            class_id: r.class_id,
            class_name: r.class_name,
            subject_name: r.subject_name,
            teacher_name: r.teacher_name,
        }).collect()
    };

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetQuizQuery { quiz_id: id };
    let quiz = ctx
        .get_quiz
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        QuizResponse::from(quiz),
        req_ctx.request_id,
    )))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishQuizCommand { quiz_id: id };
    let quiz = ctx
        .publish_quiz
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        QuizResponse::from(quiz),
        req_ctx.request_id,
    )))
}

async fn start_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<StartAttemptRequest>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = StartAttemptCommand {
        tenant_id: req_ctx.tenant_id,
        quiz_id: id,
        student_id: payload.student_id,
    };
    let attempt = ctx
        .start_attempt
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttemptResponse::from(attempt),
        req_ctx.request_id,
    )))
}

async fn submit_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, attempt_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SubmitAttemptRequest>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let answers = payload
        .answers
        .into_iter()
        .map(|a| SubmitAnswer {
            question_id: a.question_id,
            chosen_choice_id: a.chosen_choice_id,
            text_answer: a.text_answer,
        })
        .collect();

    let command = SubmitAttemptCommand {
        attempt_id,
        answers,
    };
    let attempt = ctx
        .submit_attempt
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttemptResponse::from(attempt),
        req_ctx.request_id,
    )))
}

async fn grade_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, attempt_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = GradeAttemptCommand { attempt_id };
    let attempt = ctx
        .grade_attempt
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttemptResponse::from(attempt),
        req_ctx.request_id,
    )))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct QuizChoiceResponse {
    pub id: Uuid,
    pub choice_text: String,
    pub order_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_correct: Option<bool>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct QuizQuestionResponse {
    pub id: Uuid,
    pub question_text: String,
    pub question_type: String,
    pub points: i32,
    pub order_index: i32,
    pub image_url: Option<String>,
    pub choices: Vec<QuizChoiceResponse>,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateQuizOptionInput {
    pub choice_text: String,
    pub is_correct: Option<bool>,
    pub order_index: Option<i32>,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateQuizQuestionRequest {
    pub question_text: String,
    pub question_type: Option<String>,
    pub points: Option<i32>,
    pub order_index: Option<i32>,
    pub image_url: Option<String>,
    pub choices: Option<Vec<CreateQuizOptionInput>>,
}

async fn get_questions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<QuizQuestionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru" || r.name == "Kepala Sekolah" || r.name == "Administrator")).unwrap_or(false);

    let questions = sqlx::query!(
        r#"
        SELECT id, question_text, question_type, points, order_index
        FROM quiz_questions
        WHERE quiz_id = $1
        ORDER BY order_index ASC
        "#,
        id
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let mut result = Vec::new();
    for q in questions {
        let choices = sqlx::query!(
            r#"
            SELECT id, choice_text, order_index, is_correct
            FROM quiz_choices
            WHERE question_id = $1
            ORDER BY order_index ASC
            "#,
            q.id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        result.push(QuizQuestionResponse {
            id: q.id,
            question_text: q.question_text,
            question_type: q.question_type,
            points: q.points,
            order_index: q.order_index,
            image_url: None,
            choices: choices.into_iter().map(|c| QuizChoiceResponse {
                id: c.id,
                choice_text: c.choice_text,
                order_index: c.order_index,
                is_correct: if is_teacher { Some(c.is_correct) } else { None },
            }).collect(),
        });
    }

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

async fn add_question(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateQuizQuestionRequest>,
) -> Result<Json<ApiResponse<QuizQuestionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let q_id = Uuid::new_v4();
    let q_type = payload.question_type.unwrap_or_else(|| "multiple_choice".to_string());
    let points = payload.points.unwrap_or(10);
    let order_idx = payload.order_index.unwrap_or(1);

    sqlx::query!(
        r#"
        INSERT INTO quiz_questions (id, quiz_id, question_text, question_type, points, order_index, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        "#,
        q_id,
        id,
        payload.question_text,
        q_type,
        points,
        order_idx
    )
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let mut choice_responses = Vec::new();
    if let Some(choices) = payload.choices {
        let choice_count = choices.len();
        let mut choice_ids = Vec::with_capacity(choice_count);
        let mut choice_texts = Vec::with_capacity(choice_count);
        let mut is_corrects = Vec::with_capacity(choice_count);
        let mut order_indexes = Vec::with_capacity(choice_count);

        for (idx, c) in choices.into_iter().enumerate() {
            choice_ids.push(Uuid::new_v4());
            choice_texts.push(c.choice_text);
            is_corrects.push(c.is_correct.unwrap_or(false));
            order_indexes.push(c.order_index.unwrap_or(idx as i32 + 1));
        }

        let q_ids = vec![q_id; choice_count];
        sqlx::query!(
            r#"
            INSERT INTO quiz_choices (id, question_id, choice_text, is_correct, order_index, created_at, updated_at)
            SELECT u.id, u.question_id, u.choice_text, u.is_correct, u.order_index, NOW(), NOW()
            FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::boolean[], $5::int[])
                AS u(id, question_id, choice_text, is_correct, order_index)
            "#,
            &choice_ids as &[Uuid],
            &q_ids as &[Uuid],
            &choice_texts as &[String],
            &is_corrects as &[bool],
            &order_indexes as &[i32],
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        for (idx, c_id) in choice_ids.into_iter().enumerate() {
            choice_responses.push(QuizChoiceResponse {
                id: c_id,
                choice_text: choice_texts[idx].clone(),
                order_index: order_indexes[idx],
                is_correct: Some(is_corrects[idx]),
            });
        }
    }

    let _ = sqlx::query!(
        r#"UPDATE quizzes SET questions_count = questions_count + 1 WHERE id = $1"#,
        id
    )
    .execute(&ctx.pool)
    .await;

    Ok(Json(ApiResponse::success(
        QuizQuestionResponse {
            id: q_id,
            question_text: payload.question_text,
            question_type: q_type,
            points,
            order_index: order_idx,
            image_url: payload.image_url,
            choices: choice_responses,
        },
        req_ctx.request_id,
    )))
}

