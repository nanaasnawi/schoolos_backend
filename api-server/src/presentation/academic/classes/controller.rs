use axum::{
    Json, Router,
    extract::{Query, State},
    routing::post,
};
use school_core::academic::application::class::{
    create_class::CreateClassCommand, list_classes::ListClassesQuery,
};
use school_core::common::models::page::Pagination;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateClassRequest {
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ClassResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
    pub homeroom_teacher_id: Option<Uuid>,
}

impl From<school_core::academic::domain::class::Class> for ClassResponse {
    fn from(class: school_core::academic::domain::class::Class) -> Self {
        Self {
            id: class.id,
            academic_year_id: class.academic_year_id,
            grade_level_id: class.grade_level_id,
            name: class.name,
            homeroom_teacher_id: class.homeroom_teacher_id,
        }
    }
}

pub fn class_routes() -> Router<ApplicationContext> {
    Router::new().route("/", post(create).get(list))
}

#[utoipa::path(
    post,
    operation_id = "createClass",
    path = "/api/v1/academic/classes",
    request_body = CreateClassRequest,
    responses(
        (status = 201, description = "Class created", body = ApiResponse<ClassResponse>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateClassRequest>,
) -> Result<Json<ApiResponse<ClassResponse>>, crate::error::ApiError> {
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

    let command = CreateClassCommand {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: payload.academic_year_id,
        grade_level_id: payload.grade_level_id,
        name: payload.name,
    };

    let class = ctx.create_class.execute(command).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        ClassResponse::from(class),
        meta,
        req_ctx.request_id,
    )))
}

#[derive(Deserialize)]
pub struct ListClassesParams {
    pub academic_year_id: Option<Uuid>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[utoipa::path(
    get,
    operation_id = "listClasses",
    path = "/api/v1/academic/classes",
    params(
        ("academic_year_id" = Option<Uuid>, Query, description = "Filter by academic year"),
        ("page" = Option<u64>, Query, description = "Page number"),
        ("page_size" = Option<u64>, Query, description = "Items per page")
    ),
    responses(
        (status = 200, description = "List of Classes", body = ApiResponse<Vec<ClassResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListClassesParams>,
) -> Result<Json<ApiResponse<Vec<ClassResponse>>>, crate::error::ApiError> {
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

    let query = ListClassesQuery {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: params.academic_year_id,
        pagination: Pagination {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
        },
    };

    let page_result = ctx.list_classes.execute(query).await?;

    let items = page_result
        .items
        .into_iter()
        .map(ClassResponse::from)
        .collect();

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
