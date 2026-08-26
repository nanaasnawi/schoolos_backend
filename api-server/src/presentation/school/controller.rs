use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{school_response::{SchoolPublicInfo, SchoolResponse}, update_school_request::UpdateSchoolRequest};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

#[derive(Deserialize)]
pub struct SchoolInfoQuery {
    pub npsn: Option<String>,
}

/// Public endpoint — returns basic school info (name, logo) without authentication.
/// Used by the mobile app login screen to show the school identity before login.
pub async fn get_school_public_info(
    State(ctx): State<ApplicationContext>,
    Query(params): Query<SchoolInfoQuery>,
) -> Result<Json<ApiResponse<SchoolPublicInfo>>, ApiError> {
    // Single query: filter by NPSN if provided, otherwise return first active school
    let row = sqlx::query!(
        r#"
        SELECT s.name, s.logo_url, s.npsn
        FROM schools s
        WHERE s.deleted_at IS NULL
          AND ($1::text IS NULL OR s.npsn = $1)
        LIMIT 1
        "#,
        params.npsn.as_deref()
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
        &uuid::Uuid::nil().to_string(),
    ))?;

    let data = if let Some(r) = row {
        SchoolPublicInfo {
            name: r.name,
            logo_url: r.logo_url,
            npsn: r.npsn,
        }
    } else {
        SchoolPublicInfo {
            name: "School OS".to_string(),
            logo_url: None,
            npsn: None,
        }
    };

    Ok(Json(ApiResponse::success(data, uuid::Uuid::nil().to_string())))
}

pub fn school_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/profile", get(get_current_school_profile).put(update_current_school_profile))
        .route("/{id}", get(get_school).put(update_school))
}

/// Retrieve current tenant school profile from PostgreSQL database.
#[utoipa::path(
    get,
    operation_id = "getCurrentSchoolProfile",
    path = "/api/v1/schools/profile",
    responses(
        (status = 200, description = "School profile retrieved successfully", body = inline(ApiResponse<SchoolResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "School"
)]
pub async fn get_current_school_profile(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<SchoolResponse>>, ApiError> {
    tracing::error!("get_current_school_profile CALLED WITH tenant_id: {}", req_ctx.tenant_id);
    let row = sqlx::query!(
        r#"
        SELECT id, tenant_id, name, npsn, address, phone_number, email, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        FROM schools
        WHERE tenant_id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let response_data = if let Some(r) = row {
        SchoolResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            name: r.name,
            npsn: r.npsn,
            logo_url: r.logo_url,
            address: r.address,
            phone_number: r.phone_number,
            email: r.email,
            status: r.status,
            dapodik_url: r.dapodik_url,
            dapodik_token: r.dapodik_token,
            accreditation: r.accreditation,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    } else {
        // Multi-Tenant Isolation: Fetch tenant name & NPSN from tenants table
        let tenant_info = sqlx::query!(
            r#"SELECT name, npsn FROM tenants WHERE id = $1"#,
            req_ctx.tenant_id
        )
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        let default_name = tenant_info.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "Nama Sekolah".to_string());
        let default_npsn = tenant_info.as_ref().and_then(|t| t.npsn.clone());

        let new_id = Uuid::now_v7();
        let inserted = sqlx::query!(
            r#"
            INSERT INTO schools (id, tenant_id, name, npsn, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'Active', NOW(), NOW())
            RETURNING id, tenant_id, name, npsn, address, phone_number, email, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
            "#,
            new_id,
            req_ctx.tenant_id,
            default_name,
            default_npsn
        )
        .fetch_one(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

        SchoolResponse {
            id: inserted.id,
            tenant_id: inserted.tenant_id,
            name: inserted.name,
            npsn: inserted.npsn,
            logo_url: inserted.logo_url,
            address: inserted.address,
            phone_number: inserted.phone_number,
            email: inserted.email,
            status: inserted.status,
            dapodik_url: inserted.dapodik_url,
            dapodik_token: inserted.dapodik_token,
            accreditation: inserted.accreditation,
            created_at: inserted.created_at.to_rfc3339(),
            updated_at: inserted.updated_at.to_rfc3339(),
        }
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

/// Update current tenant school profile & dynamic logo in PostgreSQL database.
#[utoipa::path(
    put,
    operation_id = "updateCurrentSchoolProfile",
    path = "/api/v1/schools/profile",
    request_body = UpdateSchoolRequest,
    responses(
        (status = 200, description = "School profile updated successfully in PostgreSQL", body = inline(ApiResponse<SchoolResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "School"
)]
pub async fn update_current_school_profile(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<UpdateSchoolRequest>,
) -> Result<Json<ApiResponse<SchoolResponse>>, ApiError> {
    let existing = sqlx::query!(
        r#"SELECT id FROM schools WHERE tenant_id = $1 AND deleted_at IS NULL LIMIT 1"#,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let school_id = if let Some(e) = existing {
        e.id
    } else {
        Uuid::now_v7()
    };

    let updated = sqlx::query!(
        r#"
        INSERT INTO schools (id, tenant_id, name, npsn, logo_url, address, phone_number, email, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at)
        VALUES ($1, $2, COALESCE($3, 'Nama Sekolah'), $4, $5, $6, $7, $8, COALESCE($9, 'Active'), $10, $11, $12, NOW(), NOW())
        ON CONFLICT (tenant_id) DO UPDATE SET
            name = COALESCE(EXCLUDED.name, schools.name),
            npsn = COALESCE(EXCLUDED.npsn, schools.npsn),
            logo_url = EXCLUDED.logo_url,
            address = COALESCE(EXCLUDED.address, schools.address),
            phone_number = COALESCE(EXCLUDED.phone_number, schools.phone_number),
            email = COALESCE(EXCLUDED.email, schools.email),
            status = COALESCE(EXCLUDED.status, schools.status),
            dapodik_url = COALESCE(EXCLUDED.dapodik_url, schools.dapodik_url),
            dapodik_token = COALESCE(EXCLUDED.dapodik_token, schools.dapodik_token),
            accreditation = COALESCE(EXCLUDED.accreditation, schools.accreditation),
            updated_at = NOW()
        RETURNING id, tenant_id, name, npsn, address, phone_number, email, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        "#,
        school_id,
        req_ctx.tenant_id,
        payload.name,
        payload.npsn,
        payload.logo_url,
        payload.address,
        payload.phone_number,
        payload.email,
        payload.status,
        payload.dapodik_url,
        payload.dapodik_token,
        payload.accreditation
    )
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let response_data = SchoolResponse {
        id: updated.id,
        tenant_id: updated.tenant_id,
        name: updated.name,
        npsn: updated.npsn,
        logo_url: updated.logo_url,
        address: updated.address,
        phone_number: updated.phone_number,
        email: updated.email,
        status: updated.status,
        dapodik_url: updated.dapodik_url,
        dapodik_token: updated.dapodik_token,
        accreditation: updated.accreditation,
        created_at: updated.created_at.to_rfc3339(),
        updated_at: updated.updated_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

/// Retrieve school details by ID.
#[utoipa::path(
    get,
    operation_id = "getSchool",
    path = "/api/v1/schools/{id}",
    params(
        ("id" = Uuid, Path, description = "School UUID")
    ),
    responses(
        (status = 200, description = "School retrieved successfully", body = inline(ApiResponse<SchoolResponse>)),
        (status = 404, description = "School not found")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "School"
)]
pub async fn get_school(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SchoolResponse>>, ApiError> {
    let row = sqlx::query!(
        r#"
        SELECT id, tenant_id, name, npsn, address, phone_number, email, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        FROM schools
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        "#,
        id,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    if let Some(r) = row {
        let response_data = SchoolResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            name: r.name,
            npsn: r.npsn,
            logo_url: r.logo_url,
            address: r.address,
            phone_number: r.phone_number,
            email: r.email,
            status: r.status,
            dapodik_url: r.dapodik_url,
            dapodik_token: r.dapodik_token,
            accreditation: r.accreditation,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        };

        return Ok(Json(ApiResponse::success(
            response_data,
            req_ctx.request_id,
        )));
    }

    Err(ApiError::new(
        ApplicationError::NotFound(ErrorCode::SchoolNotFound, "School not found".to_string()),
        &req_ctx.request_id,
    ))
}

/// Update school details by ID.
pub async fn update_school(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(_id): Path<Uuid>,
    Json(payload): Json<UpdateSchoolRequest>,
) -> Result<Json<ApiResponse<SchoolResponse>>, ApiError> {
    update_current_school_profile(State(ctx), req_ctx, Json(payload)).await
}
