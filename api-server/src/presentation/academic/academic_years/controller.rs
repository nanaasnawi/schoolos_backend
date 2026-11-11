use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use school_core::academic::application::academic_year::create_academic_year::CreateAcademicYearCommand;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

fn parse_flexible_date(s: &str) -> Option<NaiveDate> {
    let clean = s.trim();
    if clean.len() >= 10 {
        NaiveDate::parse_from_str(&clean[..10], "%Y-%m-%d").ok()
    } else {
        None
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateAcademicYearRequest {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Deserialize)]
pub struct UpdateAcademicYearRequest {
    pub name: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_active: Option<bool>,
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
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).put(update).patch(update))
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

    let start_date = parse_flexible_date(&payload.start_date).ok_or_else(|| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Format tanggal mulai tidak valid, gunakan format YYYY-MM-DD".to_string(),
                ),
            ),
            &req_ctx.request_id,
        )
    })?;

    let end_date = parse_flexible_date(&payload.end_date).ok_or_else(|| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Format tanggal selesai tidak valid, gunakan format YYYY-MM-DD".to_string(),
                ),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateAcademicYearCommand {
        tenant_id: req_ctx.tenant_id,
        name: payload.name,
        start_date,
        end_date,
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

    let rows = sqlx::query(
        r#"
        SELECT id, name, start_date, end_date, is_active
        FROM academic_years
        WHERE (tenant_id = $1 OR tenant_id = 'b158ec8a-6ea3-468a-bed5-917e37499550'::uuid) 
          AND deleted_at IS NULL
        ORDER BY is_active DESC, start_date DESC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let items: Vec<AcademicYearResponse> = rows
        .into_iter()
        .map(|r| AcademicYearResponse {
            id: r.get("id"),
            name: r.get("name"),
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            is_active: r.get("is_active"),
        })
        .collect();

    let total = items.len() as u64;
    let meta = ApiMeta {
        pagination: Some(PaginationMeta {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
            total_items: total,
            total_pages: 1,
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
) -> Result<Json<ApiResponse<AcademicYearResponse>>, crate::error::ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, name, start_date, end_date, is_active
        FROM academic_years
        WHERE id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::AcademicYearNotFound,
                format!("Academic year {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let resp = AcademicYearResponse {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
        is_active: row.get("is_active"),
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAcademicYearRequest>,
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

    let start_date = payload.start_date.as_deref().and_then(parse_flexible_date);
    let end_date = payload.end_date.as_deref().and_then(parse_flexible_date);

    let row = sqlx::query(
        r#"
        UPDATE academic_years
        SET 
            name = COALESCE($2, name),
            start_date = COALESCE($3, start_date),
            end_date = COALESCE($4, end_date),
            is_active = COALESCE($5, is_active),
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING id, name, start_date, end_date, is_active
        "#,
    )
    .bind(id)
    .bind(payload.name)
    .bind(start_date)
    .bind(end_date)
    .bind(payload.is_active)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        crate::error::ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::AcademicYearNotFound,
                format!("Academic year {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let resp = AcademicYearResponse {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
        is_active: row.get("is_active"),
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}
