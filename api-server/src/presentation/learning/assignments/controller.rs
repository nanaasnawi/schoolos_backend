use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    assignment_response::AssignmentResponse, create_assignment_request::CreateAssignmentRequest,
    grade_submission_request::GradeSubmissionRequest, submission_response::SubmissionResponse,
    submit_assignment_request::SubmitAssignmentRequest,
    update_assignment_request::UpdateAssignmentRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::assignment::{
    archive_assignment::ArchiveAssignmentCommand, close_assignment::CloseAssignmentCommand,
    create_assignment::CreateAssignmentCommand, delete_assignment::DeleteAssignmentCommand,
    get_submissions::GetSubmissionsQuery,
    grade_submission::GradeSubmissionCommand, publish_assignment::PublishAssignmentCommand,
    submit_assignment::SubmitAssignmentCommand, update_assignment::UpdateAssignmentCommand,
};

pub fn assignment_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/publish", post(publish))
        .route("/{id}/close", post(close))
        .route("/{id}/archive", post(archive))
        .route("/{id}/submit", post(submit))
        .route("/{id}/submissions", get(get_submissions))
        .route("/{id}/submissions/{submission_id}/grade", post(grade))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id.unwrap_or_else(Uuid::new_v4),
        title: payload.title,
        description: payload.description.clone(),
        instructions: payload.instructions,
        max_score: payload.max_score.unwrap_or(100),
        due_at: payload.due_at,
        assignment_type: payload.assignment_type,
    };

    let assignment = ctx
        .create_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar!(r#"SELECT id FROM teachers WHERE user_id = $1 LIMIT 1"#, aid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // Resolve class_id from UUID or class name string (e.g. "PAKET C10")
    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar!(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                    req_ctx.tenant_id,
                    cid_str.trim()
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
        _ => None,
    };

    // Resolve subject_id from description first part (e.g. "Ilmu Pengetahuan Alam dan Sosial (IPAS) • ...")
    let subject_id: Option<Uuid> = if let Some(ref desc) = payload.description {
        let first_part = desc.split('•').next().map(|s| s.trim()).unwrap_or("");
        if !first_part.is_empty() {
            sqlx::query_scalar!(
                r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                req_ctx.tenant_id,
                first_part
            )
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let _ = sqlx::query!(
        r#"UPDATE assignments SET class_id = $1, subject_id = $2, teacher_id = $3, created_by = $4 WHERE id = $5"#,
        target_class_id,
        subject_id,
        teacher_id,
        actor_id,
        assignment.id
    )
    .execute(&ctx.pool)
    .await;

    let (class_name, subject_name, teacher_name) = tokio::join!(
        async {
            if let Some(cid) = target_class_id {
                sqlx::query_scalar!(r#"SELECT name FROM classes WHERE id = $1"#, cid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
        async {
            if let Some(sid) = subject_id {
                sqlx::query_scalar!(r#"SELECT name FROM subjects WHERE id = $1"#, sid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
        async {
            if let Some(tid) = teacher_id {
                sqlx::query_scalar!(r#"SELECT full_name FROM teachers WHERE id = $1"#, tid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
    );

    let mut resp = AssignmentResponse::from(assignment);
    resp.class_id = target_class_id;
    resp.class_name = class_name;
    resp.subject_name = subject_name;
    resp.teacher_name = teacher_name;

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AssignmentResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
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
        .map(|a| a.roles.iter().any(|r| r.name == "Guru"))
        .unwrap_or(false);
    let is_parent = req_ctx
        .actor
        .as_ref()
        .map(|a| a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("wali") || n.contains("parent") || n.contains("guardian") || n.contains("ortu")
        }))
        .unwrap_or(false);
    let is_student = !is_parent && req_ctx
        .actor
        .as_ref()
        .map(|a| a.roles.iter().any(|r| r.name == "Siswa"))
        .unwrap_or(false);

    let items: Vec<AssignmentResponse> = if is_teacher {
        let rows = sqlx::query!(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type, a.status, a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND (
                  a.created_by = $2 
                  OR a.teacher_id IN (SELECT id FROM teachers WHERE user_id = $2)
                  OR a.teacher_id IS NULL
              )
            ORDER BY a.created_at DESC
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.id,
                tenant_id: r.tenant_id,
                lesson_id: r.lesson_id.unwrap_or_default(),
                title: r.title,
                description: r.description,
                instructions: r.instructions,
                max_score: r.max_score,
                due_at: r.due_at,
                assignment_type: r.assignment_type,
                status: r.status,
                is_active: r.is_active,
                created_at: r.created_at,
                updated_at: r.updated_at,
                class_id: r.class_id,
                class_name: r.class_name,
                subject_name: r.subject_name,
                teacher_name: r.teacher_name,
            })
            .collect()
    } else if is_student || is_parent {
        let rows = sqlx::query!(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type, a.status, a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND (
                  a.class_id IS NULL
                  OR a.class_id IN (
                      SELECT en.class_id 
                      FROM students s
                      JOIN enrollments en ON en.student_id = s.id
                      WHERE s.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                      UNION
                      SELECT en.class_id 
                      FROM guardians g
                      JOIN students s ON s.guardian_id = g.id
                      JOIN enrollments en ON en.student_id = s.id
                      WHERE g.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                  )
              )
            ORDER BY a.created_at DESC
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.id,
                tenant_id: r.tenant_id,
                lesson_id: r.lesson_id.unwrap_or_default(),
                title: r.title,
                description: r.description,
                instructions: r.instructions,
                max_score: r.max_score,
                due_at: r.due_at,
                assignment_type: r.assignment_type,
                status: r.status,
                is_active: r.is_active,
                created_at: r.created_at,
                updated_at: r.updated_at,
                class_id: r.class_id,
                class_name: r.class_name,
                subject_name: r.subject_name,
                teacher_name: r.teacher_name,
            })
            .collect()
    } else {
        let rows = sqlx::query!(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type, a.status, a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
                c.name as "class_name?",
                sub.name as "subject_name?",
                t.full_name as "teacher_name?"
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 AND a.deleted_at IS NULL
            ORDER BY a.created_at DESC
            "#,
            req_ctx.tenant_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.id,
                tenant_id: r.tenant_id,
                lesson_id: r.lesson_id.unwrap_or_default(),
                title: r.title,
                description: r.description,
                instructions: r.instructions,
                max_score: r.max_score,
                due_at: r.due_at,
                assignment_type: r.assignment_type,
                status: r.status,
                is_active: r.is_active,
                created_at: r.created_at,
                updated_at: r.updated_at,
                class_id: r.class_id,
                class_name: r.class_name,
                subject_name: r.subject_name,
                teacher_name: r.teacher_name,
            })
            .collect()
    };

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let row = sqlx::query!(
        r#"
        SELECT 
            a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
            a.max_score, a.due_at, a.assignment_type, a.status, a.is_active,
            a.created_at, a.updated_at,
            a.class_id,
            c.name as "class_name?",
            sub.name as "subject_name?",
            t.full_name as "teacher_name?"
        FROM assignments a
        LEFT JOIN classes c ON c.id = a.class_id
        LEFT JOIN subjects sub ON sub.id = a.subject_id
        LEFT JOIN teachers t ON t.id = a.teacher_id
        WHERE a.id = $1 AND a.tenant_id = $2 AND a.deleted_at IS NULL
        LIMIT 1
        "#,
        id,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    match row {
        Some(r) => {
            let resp = AssignmentResponse {
                id: r.id,
                tenant_id: r.tenant_id,
                lesson_id: r.lesson_id.unwrap_or_default(),
                title: r.title,
                description: r.description,
                instructions: r.instructions,
                max_score: r.max_score,
                due_at: r.due_at,
                assignment_type: r.assignment_type,
                status: r.status,
                is_active: r.is_active,
                created_at: r.created_at,
                updated_at: r.updated_at,
                class_id: r.class_id,
                class_name: r.class_name,
                subject_name: r.subject_name,
                teacher_name: r.teacher_name,
            };
            Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
        }
        None => Err(ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::AssignmentNotFound,
                format!("Assignment {} not found", id),
            ),
            &req_ctx.request_id,
        )),
    }
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        title: payload.title,
        description: payload.description,
        instructions: payload.instructions,
        max_score: payload.max_score,
        due_at: payload.due_at,
    };

    let assignment = ctx
        .update_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .publish_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn close(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CloseAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .close_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn archive(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = ArchiveAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .archive_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn delete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let command = DeleteAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn submit(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubmitAssignmentRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = SubmitAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        student_id,
        content: payload.content,
        file_url: payload.file_url,
    };

    let submission = ctx
        .submit_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubmissionResponse::from(submission),
        req_ctx.request_id,
    )))
}

async fn get_submissions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<SubmissionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetSubmissionsQuery { assignment_id: id };

    let submissions = ctx
        .get_submissions
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = submissions
        .into_iter()
        .map(SubmissionResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn grade(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, submission_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GradeSubmissionRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let grader_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = GradeSubmissionCommand {
        tenant_id: req_ctx.tenant_id,
        submission_id,
        score: payload.score,
        feedback: payload.feedback,
        graded_by: grader_id,
    };

    let submission = ctx
        .grade_submission
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubmissionResponse::from(submission),
        req_ctx.request_id,
    )))
}
