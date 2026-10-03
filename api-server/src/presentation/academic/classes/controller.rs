use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use school_core::academic::application::class::{
    create_class::CreateClassCommand, list_classes::ListClassesQuery,
};
use school_core::common::error::ApplicationError;
use school_core::common::models::page::Pagination;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::error::ApiError;
use crate::extractors::RequestContext;
use crate::response::{ApiMeta, ApiResponse, PaginationMeta};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ClassStudentDto {
    pub id: Uuid,
    pub full_name: String,
    pub nisn: String,
    pub gender: Option<String>,
    pub status: String,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub class_id: Uuid,
    pub class_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ClassStudentsQuery {
    pub class_name: Option<String>,
    pub class_id: Option<Uuid>,
    pub search: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateClassRequest {
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ClassResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
    pub homeroom_teacher_id: Option<Uuid>,
}

impl From<school_core::academic::domain::class::Class> for ClassResponse {
    fn from(class: school_core::academic::domain::class::Class) -> Self {
        Self {
            id: class.id,
            academic_year_id: class.academic_year_id,
            grade_level_id: class.grade_level_id,
            name: class.name,
            homeroom_teacher_id: class.homeroom_teacher_id,
        }
    }
}

pub fn class_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/students", get(list_class_students))
        .route("/{id}/students", get(get_class_students_by_id))
}

#[utoipa::path(
    post,
    operation_id = "createClass",
    path = "/api/v1/academic/classes",
    request_body = CreateClassRequest,
    responses(
        (status = 201, description = "Class created", body = ApiResponse<ClassResponse>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateClassRequest>,
) -> Result<Json<ApiResponse<ClassResponse>>, crate::error::ApiError> {
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

    let command = CreateClassCommand {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: payload.academic_year_id,
        grade_level_id: payload.grade_level_id,
        name: payload.name,
    };

    let class = ctx.create_class.execute(command).await?;

    let meta = ApiMeta {
        pagination: None,
        cursor: None,
        execution_time_ms: None,
        next_cursor: None,
    };

    Ok(Json(ApiResponse::success_with_meta(
        ClassResponse::from(class),
        meta,
        req_ctx.request_id,
    )))
}

#[derive(Deserialize)]
pub struct ListClassesParams {
    pub academic_year_id: Option<Uuid>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub all: Option<bool>,
}

#[utoipa::path(
    get,
    operation_id = "listClasses",
    path = "/api/v1/academic/classes",
    params(
        ("academic_year_id" = Option<Uuid>, Query, description = "Filter by academic year"),
        ("page" = Option<u64>, Query, description = "Page number"),
        ("page_size" = Option<u64>, Query, description = "Items per page"),
        ("all" = Option<bool>, Query, description = "Return all classes regardless of role")
    ),
    responses(
        (status = 200, description = "List of Classes", body = ApiResponse<Vec<ClassResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListClassesParams>,
) -> Result<Json<ApiResponse<Vec<ClassResponse>>>, crate::error::ApiError> {
    let is_admin_or_staff = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                r.name == "Kepala Sekolah"
                    || r.name == "Operator/Staff"
                    || r.name == "Admin"
                    || r.name == "SuperAdmin"
                    || r.name == "Bendahara"
            })
        })
        .unwrap_or(false);

    let is_teacher = !is_admin_or_staff
        && req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles
                    .iter()
                    .any(|r| r.name == "Guru" || r.name == "Teacher")
            })
            .unwrap_or(false);
    let is_student = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles
                .iter()
                .any(|r| r.name == "Siswa" || r.name == "Student")
        })
        .unwrap_or(false);

    if !is_admin_or_staff && !is_teacher && !is_student {
        use crate::middleware::require_permission;
        use school_core::permission::domain::permission_registry::Permission;
        require_permission(&req_ctx.actor, Permission::AcademicManage)
            .or_else(|_| require_permission(&req_ctx.actor, Permission::LearningCurriculumRead))
            .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
            .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
            .map_err(|_| {
                crate::error::ApiError::new(
                    school_core::common::error::ApplicationError::Unauthorized(
                        school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                        "Insufficient permissions".to_string(),
                    ),
                    &req_ctx.request_id,
                )
            })?;
    }

    let query = ListClassesQuery {
        tenant_id: req_ctx.tenant_id,
        academic_year_id: params.academic_year_id,
        pagination: Pagination {
            page: params.page.unwrap_or(1),
            page_size: params.page_size.unwrap_or(20),
        },
    };

    let page_result = ctx.list_classes.execute(query).await?;

    let mut items: Vec<ClassResponse> = page_result
        .items
        .into_iter()
        .map(ClassResponse::from)
        .collect();

    if is_teacher && params.all != Some(true) {
        let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
        if let Ok(Some(teacher_id)) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM teachers WHERE user_id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        {
            let allowed_class_ids: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT DISTINCT c.id FROM classes c
                WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
                  AND (
                      c.homeroom_teacher_id = $2
                      OR EXISTS (
                          SELECT 1 FROM class_schedules cs
                          WHERE cs.class_id = c.id
                            AND cs.teacher_id = $2
                            AND cs.deleted_at IS NULL
                      )
                  )
                "#,
            )
            .bind(req_ctx.tenant_id)
            .bind(teacher_id)
            .fetch_all(&ctx.pool)
            .await
            .unwrap_or_default();

            items.retain(|c| allowed_class_ids.contains(&c.id));
        }
    }

    let meta = ApiMeta {
        pagination: Some(PaginationMeta {
            page: page_result.page,
            page_size: page_result.page_size,
            total_items: page_result.total_items,
            total_pages: page_result.total_pages,
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

async fn list_class_students(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ClassStudentsQuery>,
) -> Result<Json<ApiResponse<Vec<ClassStudentDto>>>, ApiError> {
    let tenant_id = req_ctx.tenant_id;

    let is_admin_or_staff = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                r.name == "Kepala Sekolah"
                    || r.name == "Operator/Staff"
                    || r.name == "Admin"
                    || r.name == "SuperAdmin"
                    || r.name == "Bendahara"
            })
        })
        .unwrap_or(false);

    let is_teacher = !is_admin_or_staff
        && req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles
                    .iter()
                    .any(|r| r.name == "Guru" || r.name == "Teacher")
            })
            .unwrap_or(false);

    let mut teacher_filter_id: Option<Uuid> = None;
    if is_teacher {
        let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
        if let Ok(Some(t_id)) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM teachers WHERE user_id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        {
            teacher_filter_id = Some(t_id);
        }
    }

    let dtos = sqlx::query_as::<_, ClassStudentDto>(
        r#"
        SELECT 
            s.id, s.full_name, s.nisn, s.gender, s.status, s.no_hp, s.email,
            c.id as class_id, c.name as class_name
        FROM students s
        JOIN enrollments en ON en.student_id = s.id
        JOIN classes c ON c.id = en.class_id
        WHERE c.tenant_id = $1
          AND ($2::uuid IS NULL OR c.id = $2)
          AND ($3::text IS NULL OR $3 = '' OR c.name ILIKE '%' || $3 || '%')
          AND (
              $4::text IS NULL OR $4 = '' OR 
              s.full_name ILIKE '%' || $4 || '%' OR 
              s.nisn ILIKE '%' || $4 || '%'
          )
          AND (
              $5::uuid IS NULL OR
              c.homeroom_teacher_id = $5 OR
              EXISTS (
                  SELECT 1 FROM class_schedules cs
                  WHERE cs.class_id = c.id
                    AND cs.teacher_id = $5
                    AND cs.deleted_at IS NULL
              )
          )
        ORDER BY s.full_name ASC
        "#,
    )
    .bind(tenant_id)
    .bind(query.class_id)
    .bind(query.class_name.as_deref().map(|s| s.trim()))
    .bind(query.search.as_deref().map(|s| s.trim()))
    .bind(teacher_filter_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}

async fn get_class_students_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<ClassStudentDto>>>, ApiError> {
    let dtos = sqlx::query_as::<_, ClassStudentDto>(
        r#"
        SELECT 
            s.id, s.full_name, s.nisn, s.gender, s.status, s.no_hp, s.email,
            c.id as class_id, c.name as class_name
        FROM students s
        JOIN enrollments en ON en.student_id = s.id
        JOIN classes c ON c.id = en.class_id
        WHERE c.id = $1
        ORDER BY s.full_name ASC
        "#,
    )
    .bind(id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}
