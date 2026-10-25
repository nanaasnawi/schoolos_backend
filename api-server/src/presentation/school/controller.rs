use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    school_response::{SchoolPublicInfo, SchoolResponse},
    update_school_request::UpdateSchoolRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

#[derive(Deserialize)]
pub struct SchoolInfoQuery {
    pub npsn: Option<String>,
    pub tenant_id: Option<Uuid>,
}

fn row_to_school_response(r: &sqlx::postgres::PgRow) -> SchoolResponse {
    let created_at: chrono::DateTime<chrono::Utc> = r
        .try_get("created_at")
        .unwrap_or_else(|_| chrono::Utc::now());
    let updated_at: chrono::DateTime<chrono::Utc> = r
        .try_get("updated_at")
        .unwrap_or_else(|_| chrono::Utc::now());

    SchoolResponse {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        name: r.get("name"),
        npsn: r.try_get("npsn").ok().flatten(),
        logo_url: r.try_get("logo_url").ok().flatten(),
        address: r.try_get("address").ok().flatten(),
        village: r.try_get("village").ok().flatten(),
        district: r.try_get("district").ok().flatten(),
        regency: r.try_get("regency").ok().flatten(),
        province: r.try_get("province").ok().flatten(),
        postal_code: r.try_get("postal_code").ok().flatten(),
        phone_number: r.try_get("phone_number").ok().flatten(),
        email: r.try_get("email").ok().flatten(),
        website: r.try_get("website").ok().flatten(),
        headmaster_name: r.try_get("headmaster_name").ok().flatten(),
        headmaster_nip: r.try_get("headmaster_nip").ok().flatten(),
        status: r.get("status"),
        dapodik_url: r.try_get("dapodik_url").ok().flatten(),
        dapodik_token: r.try_get("dapodik_token").ok().flatten(),
        accreditation: r.try_get("accreditation").ok().flatten(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    }
}

/// Public endpoint — returns basic school info (name, logo) without authentication.
/// Used by the mobile app login screen to show the school identity before login.
pub async fn get_school_public_info(
    State(ctx): State<ApplicationContext>,
    Query(params): Query<SchoolInfoQuery>,
) -> Result<Json<ApiResponse<SchoolPublicInfo>>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT s.name, s.logo_url, s.npsn
        FROM schools s
        WHERE s.deleted_at IS NULL
          AND ($1::text IS NULL OR s.npsn = $1)
          AND ($2::uuid IS NULL OR s.tenant_id = $2)
        ORDER BY 
          (SELECT count(*) FROM students st WHERE st.tenant_id = s.tenant_id) DESC,
          s.updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(params.npsn.as_deref())
    .bind(params.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &uuid::Uuid::nil().to_string(),
        )
    })?;

    let data = if let Some(ref r) = row {
        SchoolPublicInfo {
            name: r.get("name"),
            logo_url: r.try_get("logo_url").ok().flatten(),
            npsn: r.try_get("npsn").ok().flatten(),
        }
    } else {
        SchoolPublicInfo {
            name: "School OS".to_string(),
            logo_url: None,
            npsn: None,
        }
    };

    Ok(Json(ApiResponse::success(
        data,
        uuid::Uuid::nil().to_string(),
    )))
}

pub fn school_routes() -> Router<ApplicationContext> {
    Router::new()
        .route(
            "/profile",
            get(get_current_school_profile).put(update_current_school_profile),
        )
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
    tracing::debug!(
        "get_current_school_profile called with tenant_id: {}",
        req_ctx.tenant_id
    );
    let row = sqlx::query(
        r#"
        SELECT id, tenant_id, name, npsn, address, village, district, regency, province, postal_code, phone_number, email, website, headmaster_name, headmaster_nip, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        FROM schools
        WHERE tenant_id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let response_data = if let Some(ref r) = row {
        let raw_name: String = r.get("name");
        let final_school_name = if raw_name.trim().is_empty() {
            sqlx::query_scalar::<_, String>("SELECT name FROM tenants WHERE id = $1")
                .bind(req_ctx.tenant_id)
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "Nama Sekolah".to_string())
        } else {
            raw_name
        };

        let mut res = row_to_school_response(r);
        res.name = final_school_name;
        res
    } else {
        // Multi-Tenant Isolation: Fetch tenant name & NPSN from tenants table
        let tenant_info = sqlx::query(
            r#"SELECT name, npsn FROM tenants WHERE id = $1"#,
        )
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        let default_name = tenant_info
            .as_ref()
            .map(|t| t.get::<String, _>("name"))
            .unwrap_or_else(|| "Nama Sekolah".to_string());
        let default_npsn: Option<String> = tenant_info
            .as_ref()
            .and_then(|t| t.try_get("npsn").ok().flatten());

        let new_id = Uuid::now_v7();
        let inserted = sqlx::query(
            r#"
            INSERT INTO schools (id, tenant_id, name, npsn, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'Active', NOW(), NOW())
            RETURNING id, tenant_id, name, npsn, address, village, district, regency, province, postal_code, phone_number, email, website, headmaster_name, headmaster_nip, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
            "#,
        )
        .bind(new_id)
        .bind(req_ctx.tenant_id)
        .bind(default_name)
        .bind(default_npsn)
        .fetch_one(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

        row_to_school_response(&inserted)
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
    let existing = sqlx::query(
        r#"SELECT id FROM schools WHERE tenant_id = $1 AND deleted_at IS NULL LIMIT 1"#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let school_id = if let Some(ref e) = existing {
        e.get("id")
    } else {
        Uuid::now_v7()
    };

    let name_trimmed = payload
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    let npsn_trimmed = payload
        .npsn
        .as_ref()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());

    if let Some(ref n) = name_trimmed {
        let _ = sqlx::query("UPDATE tenants SET name = $1, updated_at = NOW() WHERE id = $2")
            .bind(n)
            .bind(req_ctx.tenant_id)
            .execute(&ctx.pool)
            .await;
    }
    if let Some(ref n) = npsn_trimmed {
        let _ = sqlx::query("UPDATE tenants SET npsn = $1, updated_at = NOW() WHERE id = $2")
            .bind(n)
            .bind(req_ctx.tenant_id)
            .execute(&ctx.pool)
            .await;
    }

    let updated = sqlx::query(
        r#"
        INSERT INTO schools (
            id, tenant_id, name, npsn, logo_url, address, village, district, regency, province, postal_code,
            phone_number, email, website, headmaster_name, headmaster_nip, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        )
        VALUES (
            $1, $2, COALESCE($3, 'Nama Sekolah'), $4, $5, $6, $7, $8, $9, $10, $11,
            $12, $13, $14, $15, $16, COALESCE($17, 'Active'), $18, $19, $20, NOW(), NOW()
        )
        ON CONFLICT (tenant_id) DO UPDATE SET
            name = COALESCE(EXCLUDED.name, schools.name),
            npsn = COALESCE(EXCLUDED.npsn, schools.npsn),
            logo_url = EXCLUDED.logo_url,
            address = COALESCE(EXCLUDED.address, schools.address),
            village = COALESCE(EXCLUDED.village, schools.village),
            district = COALESCE(EXCLUDED.district, schools.district),
            regency = COALESCE(EXCLUDED.regency, schools.regency),
            province = COALESCE(EXCLUDED.province, schools.province),
            postal_code = COALESCE(EXCLUDED.postal_code, schools.postal_code),
            phone_number = COALESCE(EXCLUDED.phone_number, schools.phone_number),
            email = COALESCE(EXCLUDED.email, schools.email),
            website = COALESCE(EXCLUDED.website, schools.website),
            headmaster_name = COALESCE(EXCLUDED.headmaster_name, schools.headmaster_name),
            headmaster_nip = COALESCE(EXCLUDED.headmaster_nip, schools.headmaster_nip),
            status = COALESCE(EXCLUDED.status, schools.status),
            dapodik_url = COALESCE(EXCLUDED.dapodik_url, schools.dapodik_url),
            dapodik_token = COALESCE(EXCLUDED.dapodik_token, schools.dapodik_token),
            accreditation = COALESCE(EXCLUDED.accreditation, schools.accreditation),
            updated_at = NOW()
        RETURNING id, tenant_id, name, npsn, address, village, district, regency, province, postal_code, phone_number, email, website, headmaster_name, headmaster_nip, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        "#,
    )
    .bind(school_id)
    .bind(req_ctx.tenant_id)
    .bind(name_trimmed)
    .bind(npsn_trimmed)
    .bind(payload.logo_url)
    .bind(payload.address)
    .bind(payload.village)
    .bind(payload.district)
    .bind(payload.regency)
    .bind(payload.province)
    .bind(payload.postal_code)
    .bind(payload.phone_number)
    .bind(payload.email)
    .bind(payload.website)
    .bind(payload.headmaster_name)
    .bind(payload.headmaster_nip)
    .bind(payload.status)
    .bind(payload.dapodik_url)
    .bind(payload.dapodik_token)
    .bind(payload.accreditation)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let response_data = row_to_school_response(&updated);

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
        ("id" = Uuid, Path, description = "School ID")
    ),
    responses(
        (status = 200, description = "School retrieved successfully", body = inline(ApiResponse<SchoolResponse>)),
        (status = 404, description = "School not found", body = inline(ApiResponse<()>)),
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
    let row = sqlx::query(
        r#"
        SELECT id, tenant_id, name, npsn, address, village, district, regency, province, postal_code, phone_number, email, website, headmaster_name, headmaster_nip, logo_url, status, dapodik_url, dapodik_token, accreditation, created_at, updated_at
        FROM schools
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    if let Some(ref r) = row {
        let response_data = row_to_school_response(r);
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
