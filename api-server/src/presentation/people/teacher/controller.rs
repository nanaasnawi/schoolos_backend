use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    create_teacher_request::CreateTeacherRequest, teacher_response::TeacherResponse,
    update_teacher_request::UpdateTeacherRequest,
};
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::{ApiErrorDetail, ApiMeta, ApiResponse, PaginationMeta},
};
use school_core::people::domain::teacher::Teacher;

#[derive(Deserialize)]
pub struct TeacherFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub created_at_from: Option<DateTime<Utc>>,
    pub created_at_to: Option<DateTime<Utc>>,
}

pub fn teacher_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update))
}

fn map_teacher_response(teacher: Teacher) -> TeacherResponse {
    TeacherResponse {
        id: teacher.id,
        tenant_id: teacher.tenant_id,
        user_id: teacher.user_id,
        nip: teacher.nip,
        full_name: teacher.full_name,
        nuptk: teacher.nuptk,
        jk: teacher.jk,
        tempat_lahir: teacher.tempat_lahir,
        tanggal_lahir: teacher.tanggal_lahir,
        status_kepegawaian: teacher.status_kepegawaian,
        jenis_ptk: teacher.jenis_ptk,
        agama: teacher.agama,
        alamat_jalan: teacher.alamat_jalan,
        no_hp: teacher.no_hp,
        email: teacher.email,
        subject: teacher.subject,
        is_active: teacher.is_active,
        created_at: teacher.created_at,
        updated_at: teacher.updated_at,
    }
}

#[utoipa::path(
    post,
    operation_id = "createTeacher",
    path = "/api/v1/teachers",
    tag = "Teacher",
    request_body = CreateTeacherRequest,
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "Teacher created successfully", body = inline(ApiResponse<TeacherResponse>)),
        (status = 401, description = "Unauthorized", body = ApiErrorDetail)
    )
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateTeacherRequest>,
) -> Result<Json<ApiResponse<TeacherResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::TeacherCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    if req_ctx.idempotency_key.is_none() {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Idempotency-Key header is required for this operation".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let command = school_core::people::application::teacher::create::CreateTeacherCommand {
        tenant_id: req_ctx.tenant_id,
        full_name: payload.full_name,
        nip: payload.nip,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let teacher = ctx
        .create_teacher
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_teacher_response(teacher),
        req_ctx.correlation_id,
    )))
}

#[utoipa::path(
    get,
    operation_id = "listTeachers",
    path = "/api/v1/teachers",
    tag = "Teacher",
    security(
        ("Bearer" = [])
    ),
    params(
        ("page" = Option<u32>, Query, description = "Page number"),
        ("page_size" = Option<u32>, Query, description = "Items per page"),
        ("search" = Option<String>, Query, description = "Search by name or NIP")
    ),
    responses(
        (status = 200, description = "List of teachers", body = inline(ApiResponse<Vec<TeacherResponse>>))
    )
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(filter): Query<TeacherFilter>,
) -> Result<Json<ApiResponse<Vec<super::dto::teacher_responses::TeacherSummaryResponse>>>, ApiError>
{
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::TeacherRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);

    let query = school_core::people::application::teacher::list::ListTeachersQuery {
        tenant_id: req_ctx.tenant_id,
        filter: school_core::people::application::teacher::list::TeacherFilter {
            search: filter.search,
            status: filter.status,
            created_after: filter.created_at_from,
            created_before: filter.created_at_to,
        },
        pagination: school_core::common::models::page::Pagination {
            page: page as u64,
            page_size: page_size as u64,
        },
        sort: school_core::people::application::teacher::list::Sort::default(),
    };

    let teachers_page = ctx
        .list_teachers
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data: Vec<super::dto::teacher_responses::TeacherSummaryResponse> = teachers_page
        .items
        .into_iter()
        .map(
            |summary| super::dto::teacher_responses::TeacherSummaryResponse {
                id: summary.id,
                nip: summary.nip,
                full_name: summary.full_name,
                nuptk: summary.nuptk,
                jk: summary.jk,
                tempat_lahir: summary.tempat_lahir,
                tanggal_lahir: summary.tanggal_lahir,
                status_kepegawaian: summary.status_kepegawaian,
                jenis_ptk: summary.jenis_ptk,
                agama: summary.agama,
                alamat_jalan: summary.alamat_jalan,
                no_hp: summary.no_hp,
                email: summary.email,
                subject: summary.subject,
                status: summary.status,
                updated_at: summary.updated_at,
            },
        )
        .collect();

    let mut response = ApiResponse::success(response_data, req_ctx.correlation_id.clone());
    response.meta = Some(ApiMeta {
        pagination: Some(PaginationMeta {
            page: page as u64,
            page_size: page_size as u64,
            total_items: teachers_page.total_items,
            total_pages: teachers_page.total_pages as u64,
        }),
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    });

    Ok(Json(response))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<super::dto::teacher_responses::TeacherDetailResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::TeacherRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = school_core::people::application::teacher::get::GetTeacherQuery {
        tenant_id: req_ctx.tenant_id,
        teacher_id: id,
    };

    let detail = ctx
        .get_teacher
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = super::dto::teacher_responses::TeacherDetailResponse {
        id: detail.id,
        tenant_id: detail.tenant_id,
        user_id: detail.user_id,
        nip: detail.nip,
        full_name: detail.full_name,
        nuptk: detail.nuptk,
        jk: detail.jk,
        tempat_lahir: detail.tempat_lahir,
        tanggal_lahir: detail.tanggal_lahir,
        status_kepegawaian: detail.status_kepegawaian,
        jenis_ptk: detail.jenis_ptk,
        agama: detail.agama,
        alamat_jalan: detail.alamat_jalan,
        no_hp: detail.no_hp,
        email: detail.email,
        subject: detail.subject,
        status: detail.status,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.correlation_id,
    )))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTeacherRequest>,
) -> Result<Json<ApiResponse<TeacherResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::TeacherUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = school_core::people::application::teacher::update::UpdateTeacherCommand {
        tenant_id: req_ctx.tenant_id,
        teacher_id: id,
        full_name: payload.full_name,
        nip: payload.nip,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let teacher = ctx
        .update_teacher
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_teacher_response(teacher),
        req_ctx.correlation_id,
    )))
}
