use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    create_curriculum_request::CreateCurriculumRequest, curriculum_response::CurriculumResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::curriculum::{
    create_curriculum::CreateCurriculumCommand, get_curriculum::GetCurriculumQuery,
    list_curricula::ListCurriculaQuery,
};

pub fn curriculum_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateCurriculumRequest>,
) -> Result<Json<ApiResponse<CurriculumResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningCurriculumCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateCurriculumCommand {
        tenant_id: req_ctx.tenant_id,
        code: payload.code,
        name: payload.name,
        description: payload.description,
    };

    let curriculum = ctx
        .create_curriculum
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        CurriculumResponse::from(curriculum),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<CurriculumResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningCurriculumRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListCurriculaQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let curricula = ctx
        .list_curricula
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = curricula
        .into_iter()
        .map(CurriculumResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<CurriculumResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningCurriculumRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetCurriculumQuery {
        tenant_id: req_ctx.tenant_id,
        curriculum_id: id,
    };

    let curriculum = ctx
        .get_curriculum
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        CurriculumResponse::from(curriculum),
        req_ctx.request_id,
    )))
}
