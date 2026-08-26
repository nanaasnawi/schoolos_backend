use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    create_student_request::CreateStudentRequest, student_response::StudentResponse,
    update_student_request::UpdateStudentRequest,
};
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::{ApiMeta, ApiResponse, PaginationMeta},
};
use school_core::people::application::{
    create_student::command::CreateStudentCommand, get_student_profile::query::GetStudentQuery,
    list_students::query::ListStudentsQuery, update_student::command::UpdateStudentCommand,
};
use school_core::people::domain::student::Student;

#[derive(Deserialize)]
pub struct StudentFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub grade_level_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub status: Option<String>,
    pub created_at_from: Option<DateTime<Utc>>,
    pub created_at_to: Option<DateTime<Utc>>,
}

pub fn student_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update))
}

fn map_student_response(student: Student) -> StudentResponse {
    StudentResponse {
        id: student.id,
        tenant_id: student.tenant_id,
        user_id: student.user_id,
        guardian_id: student.guardian_id,
        nisn: student.nisn,
        full_name: student.full_name,
        nik: student.nik,
        gender: student.gender,
        place_of_birth: student.place_of_birth,
        date_of_birth: student.date_of_birth,
        religion: student.religion,
        nipd: student.nipd,
        alamat_jalan: student.alamat_jalan,
        no_hp: student.no_hp,
        email: student.email,
        status: student.status.as_db_str().to_string(),
        created_at: student.created_at,
        updated_at: student.updated_at,
        class_name: None,
    }
}

/// Create a new Student.
///
/// Requires an `Idempotency-Key` header to prevent duplicate registration.
#[utoipa::path(
    post,
    operation_id = "createStudent",
    path = "/api/v1/students",
    request_body = CreateStudentRequest,
    responses(
        (status = 201, description = "Student created successfully", body = inline(ApiResponse<StudentResponse>)),
        (status = 400, description = "Idempotency key missing or invalid"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Permission Denied")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Student"
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<Json<ApiResponse<StudentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentCreate).map_err(|_| {
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

    let command = CreateStudentCommand {
        tenant_id: req_ctx.tenant_id,
        nisn: payload.nisn,
        full_name: payload.full_name,
        nik: payload.nik,
        gender: payload.gender,
        place_of_birth: payload.place_of_birth,
        date_of_birth: payload.date_of_birth,
        religion: payload.religion,
        guardian_id: payload.guardian_id,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let student = ctx
        .create_student
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_student_response(student),
        req_ctx.correlation_id,
    )))
}

/// Retrieve a paginated list of Students.
///
/// Supports filtering by search, grade_level, class_id, status, and date range.
#[utoipa::path(
        get,
        operation_id = "listStudents",
        path = "/api/v1/students",
        params(
            ("page" = Option<u32>, Query, description = "Page number (1-indexed)"),
            ("page_size" = Option<u32>, Query, description = "Number of items per page"),
            ("search" = Option<String>, Query, description = "Search query by name"),
            ("grade_level_id" = Option<Uuid>, Query, description = "Filter by grade level UUID"),
            ("class_id" = Option<Uuid>, Query, description = "Filter by class ID"),
            ("status" = Option<String>, Query, description = "Filter by status (active/inactive)"),
            ("created_at_from" = Option<DateTime<Utc>>, Query, description = "Filter records created on or after this date"),
            ("created_at_to" = Option<DateTime<Utc>>, Query, description = "Filter records created on or before this date")
        ),
    responses(
        (status = 200, description = "Students retrieved successfully", body = inline(ApiResponse<Vec<super::dto::student_responses::StudentSummaryResponse>>)),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Student"
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(filter): Query<StudentFilter>,
) -> Result<Json<ApiResponse<Vec<super::dto::student_responses::StudentSummaryResponse>>>, ApiError>
{
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentRead).map_err(|_| {
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

    let query = ListStudentsQuery {
        tenant_id: req_ctx.tenant_id,
        filter: school_core::people::application::list_students::query::StudentFilter {
            search: filter.search,
            grade_level_id: filter.grade_level_id,
            class_id: filter.class_id,
            status: filter.status.and_then(|s| match s.as_str() {
                "Active" => Some(school_core::people::domain::student::StudentStatus::Active),
                "Inactive" => Some(school_core::people::domain::student::StudentStatus::Inactive),
                "Graduated" => Some(school_core::people::domain::student::StudentStatus::Graduated),
                "Transferred" => {
                    Some(school_core::people::domain::student::StudentStatus::Transferred)
                }
                "Pending" => Some(school_core::people::domain::student::StudentStatus::Pending),
                "Archived" => Some(school_core::people::domain::student::StudentStatus::Archived),
                _ => None,
            }),
            created_after: filter.created_at_from,
            created_before: filter.created_at_to,
            ..Default::default()
        },
        pagination: school_core::common::models::page::Pagination {
            page: page as u64,
            page_size: page_size as u64,
        },
        sort: school_core::people::application::list_students::query::Sort::default(),
    };

    let students_page = ctx
        .list_students
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data: Vec<super::dto::student_responses::StudentSummaryResponse> = students_page
        .items
        .into_iter()
        .map(
            |summary| super::dto::student_responses::StudentSummaryResponse {
                id: summary.id,
                nisn: summary.nisn,
                full_name: summary.full_name,
                nik: summary.nik,
                gender: summary.gender,
                place_of_birth: summary.place_of_birth,
                date_of_birth: summary.date_of_birth,
                religion: summary.religion,
                nipd: summary.nipd,
                alamat_jalan: summary.alamat_jalan,
                no_hp: summary.no_hp,
                email: summary.email,
                status: summary.status.as_db_str().to_string(),
                class_name: summary.class_name,
                grade: summary.grade,
                updated_at: summary.updated_at,
            },
        )
        .collect();

    let mut response = ApiResponse::success(response_data, req_ctx.correlation_id.clone());
    response.meta = Some(ApiMeta {
        pagination: Some(PaginationMeta {
            page: page as u64,
            page_size: page_size as u64,
            total_items: students_page.total_items,
            total_pages: students_page.total_pages as u64,
        }),
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    });

    Ok(Json(response))
}

/// Retrieve a Student by ID.
#[utoipa::path(
    get,
    operation_id = "getStudentById",
    path = "/api/v1/students/{id}",
    params(
        ("id" = Uuid, Path, description = "Student UUID")
    ),
    responses(
        (status = 200, description = "Student retrieved successfully", body = inline(ApiResponse<super::dto::student_responses::StudentProfileResponse>)),
        (status = 404, description = "Student not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Student"
)]
async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<super::dto::student_responses::StudentProfileResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetStudentQuery {
        tenant_id: req_ctx.tenant_id,
        student_id: id,
    };

    let profile = ctx
        .get_student
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let class_name = sqlx::query_scalar::<_, String>(
        "SELECT c.name FROM enrollments e JOIN classes c ON c.id = e.class_id WHERE e.student_id = $1 AND e.status = 'Active' LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await
    .unwrap_or(None);

    let response_data = super::dto::student_responses::StudentProfileResponse {
        id: profile.student.id,
        tenant_id: profile.student.tenant_id,
        user_id: profile.student.user_id,
        guardian_id: profile.student.guardian_id,
        nisn: profile.student.nisn.clone(),
        full_name: profile.student.full_name.clone(),
        nik: profile.student.nik.clone(),
        gender: profile.student.gender.clone(),
        place_of_birth: profile.student.place_of_birth.clone(),
        date_of_birth: profile.student.date_of_birth.clone(),
        religion: profile.student.religion.clone(),
        nipd: profile.student.nipd.clone(),
        alamat_jalan: profile.student.alamat_jalan.clone(),
        no_hp: profile.student.no_hp.clone(),
        email: profile.student.email.clone(),
        status: profile.student.status.as_db_str().to_string(),
        class_name,
        grade: None,
        // Related read models — None until Academic API (Sprint 4.3)
        guardian: profile
            .guardian
            .map(|g| serde_json::to_value(g).unwrap_or_default()),
        current_class: profile
            .current_class
            .map(|c| serde_json::to_value(c).unwrap_or_default()),
        current_enrollment: profile
            .current_enrollment
            .map(|e| serde_json::to_value(e).unwrap_or_default()),
        academic_year: profile
            .academic_year
            .map(|a| serde_json::to_value(a).unwrap_or_default()),
        attendance_summary: profile
            .attendance_summary
            .map(|a| serde_json::to_value(a).unwrap_or_default()),
        latest_assessment_summary: profile
            .latest_assessment_summary
            .map(|a| serde_json::to_value(a).unwrap_or_default()),
        created_at: profile.student.created_at,
        updated_at: profile.student.updated_at,
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.correlation_id,
    )))
}

/// Update a Student by ID.
#[utoipa::path(
    patch,
    operation_id = "updateStudent",
    path = "/api/v1/students/{id}",
    request_body = UpdateStudentRequest,
    params(
        ("id" = Uuid, Path, description = "Student UUID")
    ),
    responses(
        (status = 200, description = "Student updated successfully", body = inline(ApiResponse<StudentResponse>)),
        (status = 404, description = "Student not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Student"
)]
async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateStudentRequest>,
) -> Result<Json<ApiResponse<StudentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::StudentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateStudentCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: id,
        nisn: payload.nisn,
        full_name: payload.full_name,
        nik: payload.nik,
        gender: payload.gender,
        place_of_birth: payload.place_of_birth,
        date_of_birth: payload.date_of_birth,
        religion: payload.religion,
        request_id: Some(req_ctx.correlation_id.clone()),
    };

    let student = ctx
        .update_student
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        map_student_response(student),
        req_ctx.correlation_id,
    )))
}
