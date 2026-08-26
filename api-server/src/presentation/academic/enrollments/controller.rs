use axum::{
    Json, Router,
    extract::{Query, State},
    routing::post,
};
use school_core::academic::application::enrollment::{
    enroll_student::EnrollStudentCommand, list_enrollments::ListEnrollmentsQuery,
};
use school_core::common::models::page::Pagination;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct EnrollStudentRequest {
    pub student_id: Uuid,
    pub class_id: Uuid,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct EnrollmentResponse {
    pub id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub academic_year_id: Uuid,
    pub is_active: bool,
}

impl From<school_core::academic::domain::enrollment::Enrollment> for EnrollmentResponse {
    fn from(enrollment: school_core::academic::domain::enrollment::Enrollment) -> Self {
        Self {
            id: enrollment.id,
            student_id: enrollment.student_id,
            class_id: enrollment.class_id,
            academic_year_id: enrollment.academic_year_id,
            is_active: enrollment.status == "Active",
        }
    }
}

pub fn enrollment_routes() -> Router<ApplicationContext> {
    Router::new().route("/", post(create).get(list))
}

#[utoipa::path(
    post,
    operation_id = "enrollStudent",
    path = "/api/v1/academic/enrollments",
    request_body = EnrollStudentRequest,
    responses(
        (status = 201, description = "Student enrolled", body = ApiResponse<EnrollmentResponse>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<EnrollStudentRequest>,
) -> Result<Json<ApiResponse<EnrollmentResponse>>, crate::error::ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentUpdate).map_err(|_| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = EnrollStudentCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: payload.student_id,
        class_id: payload.class_id,
    };

    let enrollment = ctx.enroll_student.execute(command).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        EnrollmentResponse::from(enrollment),
        meta,
        req_ctx.request_id,
    )))
}

#[derive(Deserialize)]
pub struct ListEnrollmentsParams {
    pub academic_year_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[utoipa::path(
    get,
    operation_id = "listEnrollments",
    path = "/api/v1/academic/enrollments",
    params(
        ("academic_year_id" = Option<Uuid>, Query, description = "Filter by academic year"),
        ("class_id" = Option<Uuid>, Query, description = "Filter by class"),
        ("student_id" = Option<Uuid>, Query, description = "Filter by student"),
        ("page" = Option<u64>, Query, description = "Page number"),
        ("page_size" = Option<u64>, Query, description = "Items per page")
    ),
    responses(
        (status = 200, description = "List of Enrollments", body = ApiResponse<Vec<EnrollmentResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListEnrollmentsParams>,
) -> Result<Json<ApiResponse<Vec<EnrollmentResponse>>>, crate::error::ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentRead).map_err(|_| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListEnrollmentsQuery {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: params.academic_year_id,
        class_id: params.class_id,
        student_id: params.student_id,
        pagination: Pagination {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
        },
    };

    let page_result = ctx.list_enrollments.execute(query).await?;

    let items = page_result
        .items
        .into_iter()
        .map(EnrollmentResponse::from)
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
