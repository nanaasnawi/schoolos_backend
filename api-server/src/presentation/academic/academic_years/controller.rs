use axum::{
    Json, Router,
    extract::{Query, State},
    routing::post,
};
use chrono::NaiveDate;
use school_core::academic::application::academic_year::{
    create_academic_year::CreateAcademicYearCommand, list_academic_years::ListAcademicYearsQuery,
};
use school_core::common::models::page::Pagination;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateAcademicYearRequest {
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AcademicYearResponse {
    pub id: Uuid,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

impl From<school_core::academic::domain::academic_year::AcademicYear> for AcademicYearResponse {
    fn from(year: school_core::academic::domain::academic_year::AcademicYear) -> Self {
        Self {
            id: year.id,
            name: year.name,
            start_date: year.start_date,
            end_date: year.end_date,
            is_active: year.is_active,
        }
    }
}

pub fn academic_year_routes() -> Router<ApplicationContext> {
    Router::new().route("/", post(create).get(list))
}

#[utoipa::path(
    post,
    operation_id = "createAcademicYear",
    path = "/api/v1/academic/academic-years",
    request_body = CreateAcademicYearRequest,
    responses(
        (status = 201, description = "Academic Year created", body = ApiResponse<AcademicYearResponse>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAcademicYearRequest>,
) -> Result<Json<ApiResponse<AcademicYearResponse>>, crate::error::ApiError> {
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

    let command = CreateAcademicYearCommand {
        tenant_id: req_ctx.tenant_id,
        name: payload.name,
        start_date: payload.start_date,
        end_date: payload.end_date,
    };

    let year = ctx.create_academic_year.execute(command).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        AcademicYearResponse::from(year),
        meta,
        req_ctx.request_id,
    )))
}

#[derive(Deserialize)]
pub struct ListAcademicYearsParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[utoipa::path(
    get,
    operation_id = "listAcademicYears",
    path = "/api/v1/academic/academic-years",
    params(
        ("page" = Option<u64>, Query, description = "Page number"),
        ("page_size" = Option<u64>, Query, description = "Items per page")
    ),
    responses(
        (status = 200, description = "List of Academic Years", body = ApiResponse<Vec<AcademicYearResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListAcademicYearsParams>,
) -> Result<Json<ApiResponse<Vec<AcademicYearResponse>>>, crate::error::ApiError> {
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

    let query = ListAcademicYearsQuery {
        tenant_id: req_ctx.tenant_id,
        pagination: Pagination {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
        },
    };

    let page_result = ctx.list_academic_years.execute(query).await?;

    let items = page_result
        .items
        .into_iter()
        .map(AcademicYearResponse::from)
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
