use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

use super::dto::{
    create_grade_level_request::CreateGradeLevelRequest, grade_level_response::GradeLevelResponse,
};
use school_core::academic::application::grade_level::{
    create_grade_level::CreateGradeLevelCommand, get_grade_level::GetGradeLevelQuery,
    list_grade_levels::ListGradeLevelsQuery,
};
use school_core::academic::domain::grade_level::GradeLevel;
use school_core::common::models::page::Pagination;

fn map_response(gl: GradeLevel) -> GradeLevelResponse {
    GradeLevelResponse {
        id: gl.id,
        tenant_id: gl.tenant_id,
        level: gl.level,
        name: gl.name,
        created_at: gl.created_at,
        updated_at: gl.updated_at,
    }
}

pub fn grade_level_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateGradeLevelRequest>,
) -> Result<Json<ApiResponse<GradeLevelResponse>>, crate::error::ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateGradeLevelCommand {
        tenant_id: req_ctx.tenant_id,
        level: payload.level,
        name: payload.name,
    };

    let grade_level = ctx.create_grade_level.execute(command).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        map_response(grade_level),
        meta,
        req_ctx.request_id,
    )))
}

#[derive(Deserialize)]
pub struct ListGradeLevelsParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListGradeLevelsParams>,
) -> Result<Json<ApiResponse<Vec<GradeLevelResponse>>>, crate::error::ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListGradeLevelsQuery {
        tenant_id: req_ctx.tenant_id,
        pagination: Pagination {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
        },
    };

    let page_result = ctx.list_grade_levels.execute(query).await?;

    let items = page_result.items.into_iter().map(map_response).collect();

    let meta = ApiMeta {
        pagination: Some(PaginationMeta {
            page: page_result.page,
            page_size: page_result.page_size,
            total_items: page_result.total_items,
            total_pages: page_result.total_pages,
        }),
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        items,
        meta,
        req_ctx.request_id,
    )))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<GradeLevelResponse>>, crate::error::ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetGradeLevelQuery {
        tenant_id: req_ctx.tenant_id,
        grade_level_id: id,
    };

    let grade_level = ctx.get_grade_level.execute(query).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        map_response(grade_level),
        meta,
        req_ctx.request_id,
    )))
}
