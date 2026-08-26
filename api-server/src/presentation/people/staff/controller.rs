use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    create_staff_request::CreateStaffRequest, staff_response::StaffResponse,
    update_staff_request::UpdateStaffRequest,
};
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::{ApiMeta, ApiResponse, PaginationMeta},
};
use school_core::people::domain::staff::Staff;

#[derive(Deserialize)]
pub struct StaffFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub created_at_from: Option<DateTime<Utc>>,
    pub created_at_to: Option<DateTime<Utc>>,
}

pub fn staff_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update))
}

fn map_staff_response(staff: Staff) -> StaffResponse {
    StaffResponse {
        id: staff.id,
        tenant_id: staff.tenant_id,
        user_id: staff.user_id,
        full_name: staff.full_name,
        nuptk: staff.nuptk,
        jk: staff.jk,
        tempat_lahir: staff.tempat_lahir,
        tanggal_lahir: staff.tanggal_lahir,
        nip: staff.nip,
        status_kepegawaian: staff.status_kepegawaian,
        jenis_ptk: staff.jenis_ptk,
        agama: staff.agama,
        alamat_jalan: staff.alamat_jalan,
        no_hp: staff.no_hp,
        email: staff.email,
        job_title: staff.job_title,
        is_active: staff.is_active,
        created_at: staff.created_at,
        updated_at: staff.updated_at,
    }
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateStaffRequest>,
) -> Result<Json<ApiResponse<StaffResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StaffCreate).map_err(|_| {
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

    let command = school_core::people::application::staff::create::CreateStaffCommand {
        tenant_id: req_ctx.tenant_id,
        full_name: payload.full_name,
        job_title: payload.job_title,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let staff = ctx
        .create_staff
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_staff_response(staff),
        req_ctx.correlation_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(filter): Query<StaffFilter>,
) -> Result<Json<ApiResponse<Vec<super::dto::staff_responses::StaffSummaryResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StaffRead).map_err(|_| {
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

    let query = school_core::people::application::staff::list::ListStaffQuery {
        tenant_id: req_ctx.tenant_id,
        filter: school_core::people::application::staff::list::StaffFilter {
            search: filter.search,
            is_active: filter.is_active,
            created_after: filter.created_at_from,
            created_before: filter.created_at_to,
        },
        pagination: school_core::common::models::page::Pagination {
            page: page as u64,
            page_size: page_size as u64,
        },
        sort: school_core::people::application::staff::list::Sort::default(),
    };

    let staff_page = ctx
        .list_staff
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data: Vec<super::dto::staff_responses::StaffSummaryResponse> = staff_page
        .items
        .into_iter()
        .map(
            |summary| super::dto::staff_responses::StaffSummaryResponse {
                id: summary.id,
                full_name: summary.full_name,
                nuptk: summary.nuptk,
                jk: summary.jk,
                tempat_lahir: summary.tempat_lahir,
                tanggal_lahir: summary.tanggal_lahir,
                nip: summary.nip,
                status_kepegawaian: summary.status_kepegawaian,
                jenis_ptk: summary.jenis_ptk,
                agama: summary.agama,
                alamat_jalan: summary.alamat_jalan,
                no_hp: summary.no_hp,
                email: summary.email,
                job_title: summary.job_title,
                is_active: summary.is_active,
                updated_at: summary.updated_at,
            },
        )
        .collect();

    let mut response = ApiResponse::success(response_data, req_ctx.correlation_id.clone());
    response.meta = Some(ApiMeta {
        pagination: Some(PaginationMeta {
            page: page as u64,
            page_size: page_size as u64,
            total_items: staff_page.total_items,
            total_pages: staff_page.total_pages as u64,
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
) -> Result<Json<ApiResponse<super::dto::staff_responses::StaffDetailResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StaffRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = school_core::people::application::staff::get::GetStaffQuery {
        tenant_id: req_ctx.tenant_id,
        staff_id: id,
    };

    let detail = ctx
        .get_staff
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = super::dto::staff_responses::StaffDetailResponse {
        id: detail.id,
        tenant_id: detail.tenant_id,
        user_id: detail.user_id,
        full_name: detail.full_name,
        nuptk: detail.nuptk,
        jk: detail.jk,
        tempat_lahir: detail.tempat_lahir,
        tanggal_lahir: detail.tanggal_lahir,
        nip: detail.nip,
        status_kepegawaian: detail.status_kepegawaian,
        jenis_ptk: detail.jenis_ptk,
        agama: detail.agama,
        alamat_jalan: detail.alamat_jalan,
        no_hp: detail.no_hp,
        email: detail.email,
        job_title: detail.job_title,
        is_active: detail.is_active,
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
    Json(payload): Json<UpdateStaffRequest>,
) -> Result<Json<ApiResponse<StaffResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StaffUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = school_core::people::application::staff::update::UpdateStaffCommand {
        tenant_id: req_ctx.tenant_id,
        staff_id: id,
        full_name: payload.full_name,
        job_title: payload.job_title,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let staff = ctx
        .update_staff
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_staff_response(staff),
        req_ctx.correlation_id,
    )))
}
