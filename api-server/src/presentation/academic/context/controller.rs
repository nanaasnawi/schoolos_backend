use axum::{
    extract::State,
    routing::{get, put},
    Json, Router,
};
use uuid::Uuid;

use super::dto::{AcademicContextResponse, UpdateAcademicContextRequest};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

pub fn academic_context_routes() -> Router<ApplicationContext> {
    Router::new().route(
        "/current",
        get(get_current_academic_context).put(update_current_academic_context),
    )
}

/// Retrieve the active academic context (Year, Semester, Status, and School SSOT).
#[utoipa::path(
    get,
    operation_id = "getCurrentAcademicContext",
    path = "/api/v1/academic/context/current",
    responses(
        (status = 200, description = "Current academic context retrieved successfully", body = inline(ApiResponse<AcademicContextResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Academic"
)]
pub async fn get_current_academic_context(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<AcademicContextResponse>>, ApiError> {
    // 1. Fetch active academic year for tenant
    let year_row = sqlx::query!(
        r#"
        SELECT id, name, start_date, end_date, current_semester, report_date
        FROM academic_years
        WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let year = match year_row {
        Some(y) => y,
        None => {
            // Fallback to latest non-deleted year
            let fallback = sqlx::query!(
                r#"
                SELECT id, name, start_date, end_date, current_semester, report_date
                FROM academic_years
                WHERE tenant_id = $1 AND deleted_at IS NULL
                ORDER BY start_date DESC
                LIMIT 1
                "#,
                req_ctx.tenant_id
            )
            .fetch_optional(&ctx.pool)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                    &req_ctx.request_id,
                )
            })?;

            if let Some(f) = fallback {
                // Activate fallback
                let _ = sqlx::query!(
                    r#"UPDATE academic_years SET is_active = true, updated_at = NOW() WHERE id = $1"#,
                    f.id
                )
                .execute(&ctx.pool)
                .await;
                f
            } else {
                // Auto-provision 2026/2027 default
                let new_id = Uuid::now_v7();
                sqlx::query!(
                    r#"
                    INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, current_semester, report_date, created_at, updated_at)
                    VALUES ($1, $2, '2026/2027', '2026-07-01', '2027-06-30', true, 1, '2026-12-19', NOW(), NOW())
                    RETURNING id, name, start_date, end_date, current_semester, report_date
                    "#,
                    new_id,
                    req_ctx.tenant_id
                )
                .fetch_one(&ctx.pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                        &req_ctx.request_id,
                    )
                })?
            }
        }
    };

    // 2. Fetch School Profile for tenant SSOT
    let school_row = sqlx::query!(
        r#"
        SELECT id, name, npsn, logo_url, headmaster_name, headmaster_nip, address, village, district, regency, province, postal_code, phone_number, email, website
        FROM schools
        WHERE tenant_id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let default_school_name = if let Some(ref s) = school_row {
        if s.name.trim().is_empty() {
            "Nama Sekolah".to_string()
        } else {
            s.name.clone()
        }
    } else {
        "Nama Sekolah".to_string()
    };

    let semester = year.current_semester;
    let is_even = semester == 2;
    let semester_type = if is_even { "EVEN" } else { "ODD" }.to_string();
    let semester_label = if is_even {
        "Semester 2 (Genap)".to_string()
    } else {
        "Semester 1 (Gasal)".to_string()
    };
    let can_graduate = is_even;
    let report_date_str = year.report_date.map(|d| d.format("%Y-%m-%d").to_string());

    let response = AcademicContextResponse {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: year.id,
        academic_year_name: year.name,
        semester,
        semester_type,
        semester_label,
        is_even_semester: is_even,
        can_graduate_or_promote: can_graduate,
        report_date: report_date_str,
        start_date: year.start_date.format("%Y-%m-%d").to_string(),
        end_date: year.end_date.format("%Y-%m-%d").to_string(),

        school_id: school_row.as_ref().map(|s| s.id),
        school_name: default_school_name,
        npsn: school_row.as_ref().and_then(|s| s.npsn.clone()),
        logo_url: school_row.as_ref().and_then(|s| s.logo_url.clone()),
        headmaster_name: school_row.as_ref().and_then(|s| s.headmaster_name.clone()),
        headmaster_nip: school_row.as_ref().and_then(|s| s.headmaster_nip.clone()),
        address: school_row.as_ref().and_then(|s| s.address.clone()),
        village: school_row.as_ref().and_then(|s| s.village.clone()),
        district: school_row.as_ref().and_then(|s| s.district.clone()),
        regency: school_row.as_ref().and_then(|s| s.regency.clone()),
        province: school_row.as_ref().and_then(|s| s.province.clone()),
        postal_code: school_row.as_ref().and_then(|s| s.postal_code.clone()),
        phone_number: school_row.as_ref().and_then(|s| s.phone_number.clone()),
        email: school_row.as_ref().and_then(|s| s.email.clone()),
        website: school_row.as_ref().and_then(|s| s.website.clone()),
    };

    Ok(Json(ApiResponse::success(response, req_ctx.request_id)))
}

/// Update active academic context (e.g. switch active academic year or semester).
#[utoipa::path(
    put,
    operation_id = "updateCurrentAcademicContext",
    path = "/api/v1/academic/context/current",
    request_body = UpdateAcademicContextRequest,
    responses(
        (status = 200, description = "Current academic context updated successfully", body = inline(ApiResponse<AcademicContextResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Academic"
)]
pub async fn update_current_academic_context(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<UpdateAcademicContextRequest>,
) -> Result<Json<ApiResponse<AcademicContextResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Insufficient permissions to manage academic context".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    // 1. If switching active academic year
    if let Some(target_year_id) = payload.academic_year_id {
        sqlx::query!(
            r#"UPDATE academic_years SET is_active = false, updated_at = NOW() WHERE tenant_id = $1"#,
            req_ctx.tenant_id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        sqlx::query!(
            r#"UPDATE academic_years SET is_active = true, updated_at = NOW() WHERE id = $1 AND tenant_id = $2"#,
            target_year_id,
            req_ctx.tenant_id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;
    }

    // 2. If updating current semester (1 or 2)
    if let Some(sem) = payload.semester {
        let valid_sem = if sem == 2 { 2 } else { 1 };
        sqlx::query!(
            r#"UPDATE academic_years SET current_semester = $1, updated_at = NOW() WHERE tenant_id = $2 AND is_active = true"#,
            valid_sem,
            req_ctx.tenant_id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;
    }

    // 3. If updating report date
    if let Some(rd) = payload.report_date {
        sqlx::query!(
            r#"UPDATE academic_years SET report_date = $1, updated_at = NOW() WHERE tenant_id = $2 AND is_active = true"#,
            rd,
            req_ctx.tenant_id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;
    }

    // Return the updated context
    get_current_academic_context(State(ctx), req_ctx).await
}
