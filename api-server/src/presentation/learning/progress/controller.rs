use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    calculate_progress_request::CalculateProgressRequest, progress_response::ProgressResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::progress::{
    calculate_progress::CalculateProgressCommand, get_progress::GetProgressQuery,
};

pub fn progress_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/calculate", post(calculate))
        .route("/{student_id}/{class_id}/{subject_id}", get(get_progress))
}

async fn calculate(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CalculateProgressRequest>,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningProgressUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CalculateProgressCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: payload.student_id,
        class_id: payload.class_id,
    };

    let progress = ctx
        .calculate_progress
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        ProgressResponse::from(progress),
        req_ctx.request_id,
    )))
}

async fn get_progress(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((student_id, class_id, subject_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningProgressRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetProgressQuery {
        student_id,
        class_id,
        subject_id,
    };

    let progress = ctx
        .get_progress
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        ProgressResponse::from(progress),
        req_ctx.request_id,
    )))
}
