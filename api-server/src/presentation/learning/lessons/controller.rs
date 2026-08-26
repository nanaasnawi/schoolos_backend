use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    create_lesson_plan_request::CreateLessonPlanRequest,
    create_lesson_request::CreateLessonRequest, lesson_plan_response::LessonPlanResponse,
    lesson_response::LessonResponse, update_lesson_request::UpdateLessonRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::lesson::{
    archive_lesson::ArchiveLessonCommand, create_lesson::CreateLessonCommand,
    create_lesson_plan::CreateLessonPlanCommand, delete_lesson::DeleteLessonCommand,
    get_lesson::GetLessonQuery, get_lesson_plan::GetLessonPlanQuery,
    list_lessons::ListLessonsQuery, publish_lesson::PublishLessonCommand,
    update_lesson::UpdateLessonCommand,
};

pub fn lesson_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/publish", post(publish))
        .route("/{id}/archive", post(archive))
        .route("/{id}/plan", post(create_plan).get(get_plan))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateLessonRequest>,
) -> Result<Json<ApiResponse<LessonResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateLessonCommand {
        tenant_id: req_ctx.tenant_id,
        syllabus_id: payload.syllabus_id,
        code: payload.code,
        title: payload.title,
        description: payload.description,
        learning_objectives: payload.learning_objectives,
        duration_minutes: payload.duration_minutes,
        order_index: payload.order_index,
        status: payload.status,
    };

    let lesson = ctx
        .create_lesson
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonResponse::from(lesson),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<LessonResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListLessonsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let lessons = ctx
        .list_lessons
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = lessons.into_iter().map(LessonResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LessonResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetLessonQuery {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
    };

    let lesson = ctx
        .get_lesson
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonResponse::from(lesson),
        req_ctx.request_id,
    )))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateLessonRequest>,
) -> Result<Json<ApiResponse<LessonResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateLessonCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
        title: payload.title,
        description: payload.description,
        learning_objectives: payload.learning_objectives,
        duration_minutes: payload.duration_minutes,
    };

    let lesson = ctx
        .update_lesson
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonResponse::from(lesson),
        req_ctx.request_id,
    )))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LessonResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishLessonCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
    };

    let lesson = ctx
        .publish_lesson
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonResponse::from(lesson),
        req_ctx.request_id,
    )))
}

async fn archive(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LessonResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = ArchiveLessonCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
    };

    let lesson = ctx
        .archive_lesson
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonResponse::from(lesson),
        req_ctx.request_id,
    )))
}

async fn delete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let command = DeleteLessonCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_lesson
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn create_plan(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateLessonPlanRequest>,
) -> Result<Json<ApiResponse<LessonPlanResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateLessonPlanCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: id,
        teaching_methods: payload.teaching_methods,
        activities_opening: payload.activities_opening,
        activities_core: payload.activities_core,
        activities_closing: payload.activities_closing,
        resources: payload.resources,
        assessment_criteria: payload.assessment_criteria,
    };

    let plan = ctx
        .create_lesson_plan
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonPlanResponse::from(plan),
        req_ctx.request_id,
    )))
}

async fn get_plan(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LessonPlanResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningLessonRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetLessonPlanQuery { lesson_id: id };

    let plan = ctx
        .get_lesson_plan
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LessonPlanResponse::from(plan),
        req_ctx.request_id,
    )))
}
