use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{create_subject_request::CreateSubjectRequest, subject_response::SubjectResponse};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::academic::application::subject::{
    create_subject::CreateSubjectCommand, get_subject::GetSubjectQuery,
    list_subjects::ListSubjectsQuery,
};

pub fn subject_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateSubjectRequest>,
) -> Result<Json<ApiResponse<SubjectResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateSubjectCommand {
        tenant_id: req_ctx.tenant_id,
        code: payload.code,
        name: payload.name,
    };

    let subject = ctx
        .create_subject
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubjectResponse::from(subject),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SubjectResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListSubjectsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let subjects = ctx
        .list_subjects
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = subjects.into_iter().map(SubjectResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SubjectResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetSubjectQuery {
        tenant_id: req_ctx.tenant_id,
        subject_id: id,
    };

    let subject = ctx
        .get_subject
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubjectResponse::from(subject),
        req_ctx.request_id,
    )))
}
