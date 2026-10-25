use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use sqlx::Row;
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

struct YearData {
    id: Uuid,
    name: String,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    current_semester: i16,
    report_date: Option<chrono::NaiveDate>,
}

fn parse_year_row(row: &sqlx::postgres::PgRow) -> YearData {
    let start_date: chrono::NaiveDate = row
        .try_get("start_date")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    let end_date: chrono::NaiveDate = row
        .try_get("end_date")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2027, 6, 30).unwrap());
    let current_semester: i16 = row
        .try_get::<Option<i16>, _>("current_semester")
        .ok()
        .flatten()
        .unwrap_or(1);
    let report_date: Option<chrono::NaiveDate> = row.try_get("report_date").ok().flatten();

    YearData {
        id: row.get("id"),
        name: row.get("name"),
        start_date,
        end_date,
        current_semester,
        report_date,
    }
}

struct SchoolData {
    id: Uuid,
    name: String,
    npsn: Option<String>,
    logo_url: Option<String>,
    headmaster_name: Option<String>,
    headmaster_nip: Option<String>,
    address: Option<String>,
    village: Option<String>,
    district: Option<String>,
    regency: Option<String>,
    province: Option<String>,
    postal_code: Option<String>,
    phone_number: Option<String>,
    email: Option<String>,
    website: Option<String>,
}

fn parse_school_row(row: &sqlx::postgres::PgRow) -> SchoolData {
    SchoolData {
        id: row.get("id"),
        name: row.try_get("name").unwrap_or_default(),
        npsn: row.try_get("npsn").ok().flatten(),
        logo_url: row.try_get("logo_url").ok().flatten(),
        headmaster_name: row.try_get("headmaster_name").ok().flatten(),
        headmaster_nip: row.try_get("headmaster_nip").ok().flatten(),
        address: row.try_get("address").ok().flatten(),
        village: row.try_get("village").ok().flatten(),
        district: row.try_get("district").ok().flatten(),
        regency: row.try_get("regency").ok().flatten(),
        province: row.try_get("province").ok().flatten(),
        postal_code: row.try_get("postal_code").ok().flatten(),
        phone_number: row.try_get("phone_number").ok().flatten(),
        email: row.try_get("email").ok().flatten(),
        website: row.try_get("website").ok().flatten(),
    }
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
    let year_row = sqlx::query(
        r#"
        SELECT id, name, start_date, end_date, current_semester, report_date
        FROM academic_years
        WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let year = match year_row {
        Some(ref y) => parse_year_row(y),
        None => {
            // Fallback to latest non-deleted year
            let fallback = sqlx::query(
                r#"
                SELECT id, name, start_date, end_date, current_semester, report_date
                FROM academic_years
                WHERE tenant_id = $1 AND deleted_at IS NULL
                ORDER BY start_date DESC
                LIMIT 1
                "#,
            )
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                    &req_ctx.request_id,
                )
            })?;

            if let Some(ref f) = fallback {
                let parsed_f = parse_year_row(f);
                // Activate fallback
                let _ = sqlx::query("UPDATE academic_years SET is_active = true, updated_at = NOW() WHERE id = $1")
                    .bind(parsed_f.id)
                    .execute(&ctx.pool)
                    .await;
                parsed_f
            } else {
                // Auto-provision 2026/2027 default
                let new_id = Uuid::now_v7();
                let inserted_year = sqlx::query(
                    r#"
                    INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, current_semester, report_date, created_at, updated_at)
                    VALUES ($1, $2, '2026/2027', '2026-07-01', '2027-06-30', true, 1, '2026-12-19', NOW(), NOW())
                    RETURNING id, name, start_date, end_date, current_semester, report_date
                    "#,
                )
                .bind(new_id)
                .bind(req_ctx.tenant_id)
                .fetch_one(&ctx.pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                        &req_ctx.request_id,
                    )
                })?;

                parse_year_row(&inserted_year)
            }
        }
    };

    // 2. Fetch School Profile for tenant SSOT
    let school_row = sqlx::query(
        r#"
        SELECT id, name, npsn, logo_url, headmaster_name, headmaster_nip, address, village, district, regency, province, postal_code, phone_number, email, website
        FROM schools
        WHERE tenant_id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let parsed_school = school_row.as_ref().map(parse_school_row);

    let default_school_name = if let Some(ref s) = parsed_school {
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

        school_id: parsed_school.as_ref().map(|s| s.id),
        school_name: default_school_name,
        npsn: parsed_school.as_ref().and_then(|s| s.npsn.clone()),
        logo_url: parsed_school.as_ref().and_then(|s| s.logo_url.clone()),
        headmaster_name: parsed_school.as_ref().and_then(|s| s.headmaster_name.clone()),
        headmaster_nip: parsed_school.as_ref().and_then(|s| s.headmaster_nip.clone()),
        address: parsed_school.as_ref().and_then(|s| s.address.clone()),
        village: parsed_school.as_ref().and_then(|s| s.village.clone()),
        district: parsed_school.as_ref().and_then(|s| s.district.clone()),
        regency: parsed_school.as_ref().and_then(|s| s.regency.clone()),
        province: parsed_school.as_ref().and_then(|s| s.province.clone()),
        postal_code: parsed_school.as_ref().and_then(|s| s.postal_code.clone()),
        phone_number: parsed_school.as_ref().and_then(|s| s.phone_number.clone()),
        email: parsed_school.as_ref().and_then(|s| s.email.clone()),
        website: parsed_school.as_ref().and_then(|s| s.website.clone()),
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
        sqlx::query("UPDATE academic_years SET is_active = false, updated_at = NOW() WHERE tenant_id = $1")
            .bind(req_ctx.tenant_id)
            .execute(&ctx.pool)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                    &req_ctx.request_id,
                )
            })?;

        sqlx::query("UPDATE academic_years SET is_active = true, updated_at = NOW() WHERE id = $1 AND tenant_id = $2")
            .bind(target_year_id)
            .bind(req_ctx.tenant_id)
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
        sqlx::query("UPDATE academic_years SET current_semester = $1, updated_at = NOW() WHERE tenant_id = $2 AND is_active = true")
            .bind(valid_sem)
            .bind(req_ctx.tenant_id)
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
        sqlx::query("UPDATE academic_years SET report_date = $1, updated_at = NOW() WHERE tenant_id = $2 AND is_active = true")
            .bind(rd)
            .bind(req_ctx.tenant_id)
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
