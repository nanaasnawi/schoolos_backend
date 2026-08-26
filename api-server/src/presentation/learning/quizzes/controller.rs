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
    list_quizzes::ListQuizzesQuery, publish_quiz::PublishQuizCommand,
    start_attempt::StartAttemptCommand, submit_attempt::SubmitAnswer,
    submit_attempt::SubmitAttemptCommand,
};

pub fn quiz_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
        .route("/{id}/publish", post(publish))
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
        lesson_id: payload.lesson_id,
        title: payload.title,
        description: payload.description,
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

    Ok(Json(ApiResponse::success(
        QuizResponse::from(quiz),
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

    let query = ListQuizzesQuery {
        tenant_id: req_ctx.tenant_id,
    };
    let quizzes = ctx
        .list_quizzes
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = quizzes.into_iter().map(QuizResponse::from).collect();
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
