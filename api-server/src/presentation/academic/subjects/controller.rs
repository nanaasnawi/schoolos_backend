use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use super::dto::{create_subject_request::CreateSubjectRequest, subject_response::SubjectResponse};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::academic::application::subject::{
    create_subject::CreateSubjectCommand, get_subject::GetSubjectQuery,
    list_subjects::ListSubjectsQuery,
};
use school_core::permission::domain::permission_registry::Permission;
use sqlx::Row;

pub fn subject_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/rapor-structure", get(list_rapor_subjects))
        .route("/{id}", get(get_by_id))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateSubjectRequest>,
) -> Result<Json<ApiResponse<SubjectResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateSubjectCommand {
        tenant_id: req_ctx.tenant_id,
        code: payload.code,
        name: payload.name,
    };

    let subject = ctx
        .create_subject
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubjectResponse::from(subject),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SubjectResponse>>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_teacher = req_ctx
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

    if !is_teacher && !is_student {
        use crate::middleware::require_permission;
        require_permission(&req_ctx.actor, Permission::AcademicManage)
            .or_else(|_| require_permission(&req_ctx.actor, Permission::LearningCurriculumRead))
            .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
            .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
            .map_err(|_| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Unauthorized(
                        school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                        "Insufficient permissions".to_string(),
                    ),
                    &req_ctx.request_id,
                )
            })?;
    }

    let items: Vec<SubjectResponse> = if is_teacher {
        // Teacher sees all active curriculum subjects in their school tenant
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT s.id, s.tenant_id, s.code, s.name, s.is_active, s.created_at, s.updated_at
            FROM subjects s
            WHERE s.tenant_id = $1 AND s.deleted_at IS NULL AND s.is_active = true
            ORDER BY s.name ASC
            "#
        )
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .map(|r| SubjectResponse {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                code: r.get("code"),
                name: r.get("name"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect()
    } else if is_student {
        // Student sees subjects that have materials for their active enrolled classes,
        // or all active tenant subjects if none yet
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT s.id, s.tenant_id, s.code, s.name, s.is_active, s.created_at, s.updated_at
            FROM subjects s
            INNER JOIN syllabuses sy ON sy.subject_id = s.id AND sy.deleted_at IS NULL
            INNER JOIN lessons l ON l.syllabus_id = sy.id AND l.deleted_at IS NULL
            INNER JOIN learning_materials m ON m.lesson_id = l.id AND m.deleted_at IS NULL
            WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
              AND m.is_active = true
              AND m.class_id IN (
                  SELECT en.class_id
                  FROM students st
                  JOIN enrollments en ON en.student_id = st.id
                  WHERE st.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
              )
            ORDER BY s.name ASC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

        if !rows.is_empty() {
            rows.into_iter()
                .map(|r| SubjectResponse {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    code: r.get("code"),
                    name: r.get("name"),
                    is_active: r.get("is_active"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                })
                .collect()
        } else {
            let fallback_rows = sqlx::query(
                r#"
                SELECT DISTINCT s.id, s.tenant_id, s.code, s.name, s.is_active, s.created_at, s.updated_at
                FROM subjects s
                WHERE s.tenant_id = $1 AND s.deleted_at IS NULL AND s.is_active = true
                ORDER BY s.name ASC
                "#
            )
            .bind(req_ctx.tenant_id)
            .fetch_all(&ctx.pool)
            .await
            .unwrap_or_default();

            fallback_rows
                .into_iter()
                .map(|r| SubjectResponse {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    code: r.get("code"),
                    name: r.get("name"),
                    is_active: r.get("is_active"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                })
                .collect()
        }
    } else {
        // Super Admin / Kepala Sekolah / Staf sees all subjects in tenant
        let query = ListSubjectsQuery {
            tenant_id: req_ctx.tenant_id,
        };

        let subjects = ctx
            .list_subjects
            .execute(query)
            .await
            .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

        subjects.into_iter().map(SubjectResponse::from).collect()
    };

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SubjectResponse>>, ApiError> {
    use crate::middleware::require_permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::LearningCurriculumRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_teacher = req_ctx
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

    // Access control: teachers and students can only see subjects tied to them
    if is_teacher || is_student {
        let has_access = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM subjects s
                WHERE s.id = $1 AND s.tenant_id = $2 AND s.deleted_at IS NULL
                  AND (
                      $3 = false AND $4 = false
                      OR $3 = true AND EXISTS(
                          SELECT 1 FROM syllabuses sy
                          INNER JOIN lessons l ON l.syllabus_id = sy.id AND l.deleted_at IS NULL
                          INNER JOIN learning_materials m ON m.lesson_id = l.id AND m.deleted_at IS NULL
                          WHERE sy.subject_id = s.id
                            AND (m.created_by = $5 OR m.teacher_id IN (SELECT id FROM teachers WHERE user_id = $5))
                      )
                      OR $4 = true AND EXISTS(
                          SELECT 1 FROM syllabuses sy
                          INNER JOIN lessons l ON l.syllabus_id = sy.id AND l.deleted_at IS NULL
                          INNER JOIN learning_materials m ON m.lesson_id = l.id AND m.deleted_at IS NULL
                          WHERE sy.subject_id = s.id
                            AND m.is_active = true
                            AND m.class_id IN (
                                SELECT en.class_id FROM students st
                                JOIN enrollments en ON en.student_id = st.id
                                WHERE st.user_id = $5 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                            )
                      )
                  )
            )
            "#
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(is_teacher)
        .bind(is_student)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !has_access {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::ResourceNotFound,
                    "Mata pelajaran tidak ditemukan".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let query = GetSubjectQuery {
        tenant_id: req_ctx.tenant_id,
        subject_id: id,
    };

    let subject = ctx
        .get_subject
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubjectResponse::from(subject),
        req_ctx.request_id,
    )))
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct RaporSubjectsQuery {
    pub student_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct RaporSubjectItem {
    pub student_id: Uuid,
    pub student_name: String,
    pub gender: Option<String>,
    pub class_id: Uuid,
    pub class_name: String,
    pub phase: String,
    pub subject_code: String,
    pub subject_name: String,
    pub subject_group: String,
    pub academic_year: String,
    pub semester: String,
}

pub async fn list_rapor_subjects(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<RaporSubjectsQuery>,
) -> Result<Json<ApiResponse<Vec<RaporSubjectItem>>>, ApiError> {
    let rows = if let Some(student_id) = query.student_id {
        sqlx::query(
            r#"
            SELECT student_id, student_name, gender, class_id, class_name, phase,
                   subject_code, subject_name, subject_group, academic_year, semester
            FROM rapor_mata_pelajaran
            WHERE tenant_id = $1 AND student_id = $2
            ORDER BY 
                CASE subject_group 
                    WHEN 'UMUM' THEN 1 
                    WHEN 'PILIHAN' THEN 2 
                    WHEN 'MUATAN_LOKAL' THEN 3 
                    ELSE 4 
                END,
                subject_name ASC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(student_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    } else if let Some(class_id) = query.class_id {
        sqlx::query(
            r#"
            SELECT student_id, student_name, gender, class_id, class_name, phase,
                   subject_code, subject_name, subject_group, academic_year, semester
            FROM rapor_mata_pelajaran
            WHERE tenant_id = $1 AND class_id = $2
            ORDER BY student_name ASC, 
                CASE subject_group 
                    WHEN 'UMUM' THEN 1 
                    WHEN 'PILIHAN' THEN 2 
                    WHEN 'MUATAN_LOKAL' THEN 3 
                    ELSE 4 
                END,
                subject_name ASC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(class_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query(
            r#"
            SELECT student_id, student_name, gender, class_id, class_name, phase,
                   subject_code, subject_name, subject_group, academic_year, semester
            FROM rapor_mata_pelajaran
            WHERE tenant_id = $1
            ORDER BY student_name ASC, 
                CASE subject_group 
                    WHEN 'UMUM' THEN 1 
                    WHEN 'PILIHAN' THEN 2 
                    WHEN 'MUATAN_LOKAL' THEN 3 
                    ELSE 4 
                END,
                subject_name ASC
            LIMIT 500
            "#
        )
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    };

    let items = rows.into_iter().map(|r| RaporSubjectItem {
        student_id: r.get("student_id"),
        student_name: r.get("student_name"),
        gender: r.get("gender"),
        class_id: r.get("class_id"),
        class_name: r.get("class_name"),
        phase: r.get("phase"),
        subject_code: r.get("subject_code"),
        subject_name: r.get("subject_name"),
        subject_group: r.get("subject_group"),
        academic_year: r.get("academic_year"),
        semester: r.get("semester"),
    }).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}
