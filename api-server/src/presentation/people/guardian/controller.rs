use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    create_guardian_request::CreateGuardianRequest, guardian_response::GuardianResponse,
    update_guardian_request::UpdateGuardianRequest,
};
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::{ApiMeta, ApiResponse, PaginationMeta},
};
use school_core::people::domain::guardian::Guardian;

#[derive(Deserialize)]
pub struct GuardianFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub created_at_from: Option<DateTime<Utc>>,
    pub created_at_to: Option<DateTime<Utc>>,
}

pub fn guardian_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/overview", get(get_guardians_overview))
        .route("/{id}", get(get_by_id).patch(update))
}

fn map_guardian_response(guardian: Guardian) -> GuardianResponse {
    GuardianResponse {
        id: guardian.id,
        tenant_id: guardian.tenant_id,
        user_id: guardian.user_id,
        full_name: guardian.full_name,
        phone_number: guardian.phone_number,
        created_at: guardian.created_at,
        updated_at: guardian.updated_at,
    }
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateGuardianRequest>,
) -> Result<Json<ApiResponse<GuardianResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::GuardianCreate).map_err(|_| {
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

    let command = school_core::people::application::guardian::create::CreateGuardianCommand {
        tenant_id: req_ctx.tenant_id,
        full_name: payload.full_name,
        phone_number: payload.phone_number,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let guardian = ctx
        .create_guardian
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_guardian_response(guardian),
        req_ctx.correlation_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(filter): Query<GuardianFilter>,
) -> Result<
    Json<ApiResponse<Vec<super::dto::guardian_detail_response::GuardianDetailResponse>>>,
    ApiError,
> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::GuardianRead).map_err(|_| {
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

    let query = school_core::people::application::guardian::list::ListGuardiansQuery {
        tenant_id: req_ctx.tenant_id,
        filter: school_core::people::application::guardian::list::GuardianFilter {
            search: filter.search,
            created_after: filter.created_at_from,
            created_before: filter.created_at_to,
        },
        pagination: school_core::common::models::page::Pagination {
            page: page as u64,
            page_size: page_size as u64,
        },
        sort: school_core::people::application::guardian::list::Sort::default(),
    };

    let guardians_page = ctx
        .list_guardians
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data: Vec<super::dto::guardian_detail_response::GuardianDetailResponse> =
        guardians_page
            .items
            .into_iter()
            .map(
                |detail| super::dto::guardian_detail_response::GuardianDetailResponse {
                    id: detail.id,
                    tenant_id: detail.tenant_id,
                    user_id: detail.user_id,
                    full_name: detail.full_name,
                    phone_number: detail.phone_number,
                    created_at: detail.created_at,
                    updated_at: detail.updated_at,
                    deleted_at: detail.deleted_at,
                },
            )
            .collect();

    let mut response = ApiResponse::success(response_data, req_ctx.correlation_id.clone());
    response.meta = Some(ApiMeta {
        pagination: Some(PaginationMeta {
            page: page as u64,
            page_size: page_size as u64,
            total_items: guardians_page.total_items,
            total_pages: guardians_page.total_pages as u64,
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
) -> Result<Json<ApiResponse<super::dto::guardian_detail_response::GuardianDetailResponse>>, ApiError>
{
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::GuardianRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = school_core::people::application::guardian::get::GetGuardianQuery {
        tenant_id: req_ctx.tenant_id,
        guardian_id: id,
    };

    let detail = ctx
        .get_guardian
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = super::dto::guardian_detail_response::GuardianDetailResponse {
        id: detail.id,
        tenant_id: detail.tenant_id,
        user_id: detail.user_id,
        full_name: detail.full_name,
        phone_number: detail.phone_number,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
        deleted_at: detail.deleted_at,
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
    Json(payload): Json<UpdateGuardianRequest>,
) -> Result<Json<ApiResponse<GuardianResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::GuardianUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = school_core::people::application::guardian::update::UpdateGuardianCommand {
        tenant_id: req_ctx.tenant_id,
        guardian_id: id,
        full_name: payload.full_name,
        phone_number: payload.phone_number,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let guardian = ctx
        .update_guardian
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_guardian_response(guardian),
        req_ctx.correlation_id,
    )))
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct GuardianOverviewDto {
    pub id: String,
    pub full_name: String,
    pub relationship: String,
    pub student_id: String,
    pub student_name: String,
    pub student_nisn: String,
    pub phone: String,
    pub is_real_data: bool,
}

async fn get_guardians_overview(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<GuardianOverviewDto>>>, ApiError> {
    struct RowResult {
        student_id: Uuid,
        student_name: String,
        student_nisn: String,
        guardian_id: Option<Uuid>,
        guardian_name: Option<String>,
        phone: Option<String>,
    }

    let records = sqlx::query_as!(
        RowResult,
        r#"
        SELECT 
            s.id as student_id,
            s.full_name as student_name,
            s.nisn as student_nisn,
            g.id as "guardian_id?",
            g.full_name as "guardian_name?",
            COALESCE(g.phone_number, s.no_hp, '') as "phone?"
        FROM students s
        LEFT JOIN guardians g ON g.id = s.guardian_id
        WHERE s.tenant_id = $1
        ORDER BY s.full_name ASC
        "#,
        req_ctx.tenant_id
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Internal(e.to_string()), &req_ctx.request_id))?;

    let dtos: Vec<GuardianOverviewDto> = records.into_iter().map(|r| {
        let has_guardian = r.guardian_name.is_some() && !r.guardian_name.as_ref().unwrap().trim().is_empty();
        GuardianOverviewDto {
            id: r.guardian_id.map(|u| u.to_string()).unwrap_or_else(|| r.student_id.to_string()),
            full_name: r.guardian_name.unwrap_or_else(|| "(Belum Ada Data Wali)".to_string()),
            relationship: if has_guardian { "Orang Tua / Wali".to_string() } else { "Belum Diisi".to_string() },
            student_id: r.student_id.to_string(),
            student_name: r.student_name,
            student_nisn: r.student_nisn,
            phone: r.phone.filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "-".to_string()),
            is_real_data: has_guardian,
        }
    }).collect();

    Ok(Json(ApiResponse::success(dtos, req_ctx.correlation_id)))
}
