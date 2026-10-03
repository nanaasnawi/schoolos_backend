use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
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
        .route("/{id}", get(get_by_id).patch(update).put(update))
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

    let mut teacher = ctx
        .create_teacher
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // Auto-provision login account for the newly registered teacher
    let clean_fullname = teacher.full_name.trim();
    let base_username = clean_fullname
        .to_lowercase()
        .replace("s.pd", "")
        .replace("m.pd", "")
        .replace("s.ag", "")
        .replace("s.kom", "")
        .replace("drs", "")
        .replace("dra", "")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(".");
    let uname = if base_username.is_empty() {
        format!("guru_{}", &teacher.id.to_string()[..8])
    } else {
        base_username
    };

    let user_email = format!("{}@guru.schoolos.id", &teacher.id.to_string().replace('-', "")[..8]);
    let default_guru_pw_hash = "$argon2id$v=19$m=65536,p=4,t=3$SnCFuF71lzKF+Cuw4svZPw$c9YgXUK8C/boJ85Pb2IEuyK1xsNP28uGdzlvvflF5ts"; // guru2565

    if let Ok(uid) = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO users (id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, true, NOW(), NOW())
        ON CONFLICT (tenant_id, email) DO UPDATE SET full_name = EXCLUDED.full_name, updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&uname)
    .bind(&user_email)
    .bind(default_guru_pw_hash)
    .bind(clean_fullname)
    .fetch_one(&ctx.pool)
    .await
    {
        if let Ok(Some(role_id)) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Guru' LIMIT 1"
        )
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        {
            let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(uid)
                .bind(role_id)
                .execute(&ctx.pool)
                .await;
        }
        let _ = sqlx::query("UPDATE teachers SET user_id = $1 WHERE id = $2")
            .bind(uid)
            .bind(teacher.id)
            .execute(&ctx.pool)
            .await;
        teacher.user_id = Some(uid);
    }

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
        full_name: payload.full_name.clone(),
        nip: payload.nip.clone(),
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let mut teacher = ctx
        .update_teacher
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // Update additional fields if provided in payload
    if payload.nuptk.is_some()
        || payload.jk.is_some()
        || payload.tempat_lahir.is_some()
        || payload.status_kepegawaian.is_some()
        || payload.jenis_ptk.is_some()
        || payload.agama.is_some()
        || payload.alamat_jalan.is_some()
        || payload.no_hp.is_some()
        || payload.email.is_some()
        || payload.subject.is_some()
        || payload.is_active.is_some()
    {
        let _ = sqlx::query(
            r#"
            UPDATE teachers
            SET nuptk = COALESCE($1, nuptk),
                jk = COALESCE($2, jk),
                tempat_lahir = COALESCE($3, tempat_lahir),
                status_kepegawaian = COALESCE($4, status_kepegawaian),
                jenis_ptk = COALESCE($5, jenis_ptk),
                agama = COALESCE($6, agama),
                alamat_jalan = COALESCE($7, alamat_jalan),
                no_hp = COALESCE($8, no_hp),
                email = COALESCE($9, email),
                subject = COALESCE($10, subject),
                is_active = COALESCE($11, is_active),
                updated_at = NOW()
            WHERE id = $12 AND tenant_id = $13
            "#
        )
        .bind(&payload.nuptk)
        .bind(&payload.jk)
        .bind(&payload.tempat_lahir)
        .bind(&payload.status_kepegawaian)
        .bind(&payload.jenis_ptk)
        .bind(&payload.agama)
        .bind(&payload.alamat_jalan)
        .bind(&payload.no_hp)
        .bind(&payload.email)
        .bind(&payload.subject)
        .bind(payload.is_active)
        .bind(id)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;
    }

    // Synchronize teacher name across users table and dependent records
    if let Some(ref new_name) = payload.full_name {
        let clean_name = new_name.trim();
        if !clean_name.is_empty() {
            if let Some(uid) = teacher.user_id {
                let _ = sqlx::query("UPDATE users SET full_name = $1, updated_at = NOW() WHERE id = $2")
                    .bind(clean_name)
                    .bind(uid)
                    .execute(&ctx.pool)
                    .await;
            } else if let Some(ref email) = teacher.email {
                let _ = sqlx::query("UPDATE users SET full_name = $1, updated_at = NOW() WHERE tenant_id = $2 AND email = $3")
                    .bind(clean_name)
                    .bind(req_ctx.tenant_id)
                    .bind(email)
                    .execute(&ctx.pool)
                    .await;
            }

            let _ = sqlx::query("UPDATE inquiry_threads SET teacher_name = $1, updated_at = NOW() WHERE teacher_id = $2")
                .bind(clean_name)
                .bind(id)
                .execute(&ctx.pool)
                .await;

            let _ = sqlx::query("UPDATE learning_materials SET teacher_name = $1 WHERE teacher_id = $2")
                .bind(clean_name)
                .bind(id)
                .execute(&ctx.pool)
                .await;
        }
    }

    // Reload refreshed teacher model
    if let Ok(Some(refreshed)) = ctx.teacher_repo.find_by_id(id).await {
        teacher = refreshed;
    }

    Ok(Json(ApiResponse::success(
        map_teacher_response(teacher),
        req_ctx.correlation_id,
    )))
}
