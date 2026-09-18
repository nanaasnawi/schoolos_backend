use axum::{
    Json, Router,
    extract::{Path, State, Multipart, DefaultBodyLimit},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;


use super::dto::{
    create_learning_material_request::CreateLearningMaterialRequest,
    learning_material_response::LearningMaterialResponse,
    update_learning_material_request::UpdateLearningMaterialRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::learning_material::{
    create_learning_material::CreateLearningMaterialCommand,
    delete_learning_material::DeleteLearningMaterialCommand,
    update_learning_material::UpdateLearningMaterialCommand,
};

pub fn material_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route(
            "/upload",
            post(upload_file).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route("/completed", get(get_completed_materials))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/toggle-complete", post(toggle_complete))
        .route("/{id}/completions", get(get_material_completions))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru" || r.name == "Teacher")).unwrap_or(false);
    if !is_teacher {
        use crate::middleware::require_permission;
        use school_core::permission::domain::permission_registry::Permission;
        require_permission(&req_ctx.actor, Permission::LearningMaterialCreate).map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;
    }

    let command = CreateLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        material_type: payload.material_type,
        title: payload.title,
        description: payload.description.clone(),
        storage_key: payload.storage_key,
        external_url: payload.external_url,
        order_index: payload.order_index,
        visibility: payload.visibility,
    };

    let material = ctx
        .create_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar!(r#"SELECT id FROM teachers WHERE user_id = $1 LIMIT 1"#, aid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let teacher_id = payload.teacher_id.or(resolved_actor_teacher_id);

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
        _ => {
            if let Some(ref desc) = payload.description {
                let parts: Vec<&str> = desc.split(" • ").collect();
                if parts.len() >= 2 {
                    let cand = parts[1].trim();
                    sqlx::query_scalar!(
                        r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                        req_ctx.tenant_id,
                        cand
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
            }
        }
    };

    let _ = sqlx::query!(
        r#"UPDATE learning_materials SET class_id = $1, teacher_id = $2, created_by = $3 WHERE id = $4"#,
        target_class_id,
        teacher_id,
        actor_id,
        material.id
    )
    .execute(&ctx.pool)
    .await;

    // Trigger in-app and FCM push notifications to enrolled students
    let teacher_name = if let Some(tid) = teacher_id {
        sqlx::query_scalar::<_, String>(
            "SELECT u.full_name FROM teachers t JOIN users u ON u.id = t.user_id WHERE t.id = $1"
        )
        .bind(tid)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Guru Pengampu".to_string())
    } else if let Some(aid) = actor_id {
        sqlx::query_scalar::<_, String>(
            "SELECT full_name FROM users WHERE id = $1"
        )
        .bind(aid)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Guru Pengampu".to_string())
    } else {
        "Guru Pengampu".to_string()
    };

    let notif_title = format!("📚 Materi Baru: {}", material.title);

    if let Some(cid) = target_class_id {
        let class_name = sqlx::query_scalar::<_, String>(
            "SELECT name FROM classes WHERE id = $1 AND tenant_id = $2"
        )
        .bind(cid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Kelas".to_string());

        let notif_body = format!(
            "{} telah menambahkan modul pembelajaran baru untuk kelas {}. Pelajari sekarang!",
            teacher_name, class_name
        );

        let _ = sqlx::query(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, is_read, created_at)
            SELECT 
                gen_random_uuid(),
                s.tenant_id,
                s.user_id,
                $1,
                $2,
                'LEARNING_MATERIAL',
                'in_app',
                FALSE,
                NOW()
            FROM students s
            JOIN enrollments en ON en.student_id = s.id
            WHERE en.class_id = $3 AND (en.status = 'Active' OR en.status = 'ACTIVE')
            "#
        )
        .bind(&notif_title)
        .bind(&notif_body)
        .bind(cid)
        .execute(&ctx.pool)
        .await;

        crate::infrastructure::fcm::trigger_fcm_push_notification(
            notif_title,
            notif_body,
            "Materi Pembelajaran".to_string(),
            material.id,
        );
    } else {
        let notif_body = format!(
            "{} telah menambahkan modul pembelajaran baru: {}. Pelajari sekarang!",
            teacher_name, material.title
        );

        let _ = sqlx::query(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, is_read, created_at)
            SELECT 
                gen_random_uuid(),
                s.tenant_id,
                s.user_id,
                $1,
                $2,
                'LEARNING_MATERIAL',
                'in_app',
                FALSE,
                NOW()
            FROM students s
            WHERE s.tenant_id = $3
            "#
        )
        .bind(&notif_title)
        .bind(&notif_body)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;

        crate::infrastructure::fcm::trigger_fcm_push_notification(
            notif_title,
            notif_body,
            "Materi Pembelajaran".to_string(),
            material.id,
        );
    }

    Ok(Json(ApiResponse::success(
        LearningMaterialResponse::from(material),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<LearningMaterialResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala sekolah") || n.contains("operator")
        })
    }).unwrap_or(false);

    let is_teacher = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "guru" || n.contains("teacher")
        })
    }).unwrap_or(false);

    let is_student = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "siswa" || n.contains("student")
        })
    }).unwrap_or(false);

    let items: Vec<LearningMaterialResponse> = if is_teacher {
        // Teacher strictly sees ONLY materials they created or are assigned to them
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, COALESCE(m.external_url, lb.file_url) as external_url,
                m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, 'Umum') as subject_name,
                m.start_page, m.end_page,
                (SELECT COUNT(*)::bigint FROM student_material_completions smc WHERE smc.material_id = m.id) as completed_count
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = m.created_by
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            WHERE m.tenant_id = $1 
              AND m.deleted_at IS NULL
              AND (
                  m.created_by = $2 
                  OR m.teacher_id IN (SELECT id FROM teachers WHERE user_id = $2)
              )
            ORDER BY m.created_at DESC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            material_type: r.get("material_type"),
            title: r.get("title"),
            description: r.get("description"),
            storage_key: r.get("storage_key"),
            external_url: r.get("external_url"),
            order_index: r.get("order_index"),
            visibility: r.get("visibility"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            is_completed: None,
            completed_count: r.get("completed_count"),
            teacher_name: r.get("teacher_name"),
            teacher_id: r.get("teacher_id"),
            class_name: r.get("class_name"),
            class_id: r.get("class_id"),
            subject_name: r.get("subject_name"),
            start_page: r.get("start_page"),
            end_page: r.get("end_page"),
        }).collect()
    } else if is_student && !is_admin {
        // Student sees materials ONLY for their active enrolled classes (Paket A / B / C strictly isolated)
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, COALESCE(m.external_url, lb.file_url) as external_url,
                m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, 'Umum') as subject_name,
                m.start_page, m.end_page,
                (smc.id IS NOT NULL) as is_completed
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = m.created_by
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            LEFT JOIN student_material_completions smc 
                ON smc.material_id = m.id 
                AND smc.student_id IN (SELECT id FROM students WHERE user_id = $2)
            WHERE m.tenant_id = $1 
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND m.class_id IN (
                  SELECT en.class_id 
                  FROM students s
                  JOIN enrollments en ON en.student_id = s.id
                  WHERE s.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
              )
            ORDER BY m.created_at DESC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            material_type: r.get("material_type"),
            title: r.get("title"),
            description: r.get("description"),
            storage_key: r.get("storage_key"),
            external_url: r.get("external_url"),
            order_index: r.get("order_index"),
            visibility: r.get("visibility"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            is_completed: Some(r.get("is_completed")),
            completed_count: None,
            teacher_name: r.get("teacher_name"),
            teacher_id: r.get("teacher_id"),
            class_name: r.get("class_name"),
            class_id: r.get("class_id"),
            subject_name: r.get("subject_name"),
            start_page: r.get("start_page"),
            end_page: r.get("end_page"),
        }).collect()
    } else if is_admin {
        // Super Admin / Kepala Sekolah / Staf sees all materials in tenant
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, COALESCE(m.external_url, lb.file_url) as external_url,
                m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, 'Umum') as subject_name,
                m.start_page, m.end_page,
                (SELECT COUNT(*)::bigint FROM student_material_completions smc WHERE smc.material_id = m.id) as completed_count
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = m.created_by
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            WHERE m.tenant_id = $1 AND m.deleted_at IS NULL
            ORDER BY m.created_at DESC
            "#
        )
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            lesson_id: r.get("lesson_id"),
            material_type: r.get("material_type"),
            title: r.get("title"),
            description: r.get("description"),
            storage_key: r.get("storage_key"),
            external_url: r.get("external_url"),
            order_index: r.get("order_index"),
            visibility: r.get("visibility"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            is_completed: None,
            completed_count: r.get("completed_count"),
            teacher_name: r.get("teacher_name"),
            teacher_id: r.get("teacher_id"),
            class_name: r.get("class_name"),
            class_id: r.get("class_id"),
            subject_name: r.get("subject_name"),
            start_page: r.get("start_page"),
            end_page: r.get("end_page"),
        }).collect()
    } else {
        vec![]
    };

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala sekolah") || n.contains("operator")
        })
    }).unwrap_or(false);

    let is_teacher = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "guru" || n.contains("teacher")
        })
    }).unwrap_or(false);

    let is_student = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "siswa" || n.contains("student")
        })
    }).unwrap_or(false);

    if is_teacher {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (created_by = $3 OR teacher_id IN (SELECT id FROM teachers WHERE user_id = $3))
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !owns {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Anda tidak memiliki hak akses untuk materi pembelajaran guru lain".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    } else if is_student && !is_admin {
        let can_access = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials m
                WHERE m.id = $1 AND m.tenant_id = $2 AND m.deleted_at IS NULL AND m.is_active = true
                  AND (
                      m.class_id IS NULL OR
                      m.class_id IN (
                          SELECT en.class_id 
                          FROM students s
                          JOIN enrollments en ON en.student_id = s.id
                          WHERE s.user_id = $3 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                      )
                  )
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !can_access {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Materi ini tidak tersedia untuk kelas Anda".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let row_opt = sqlx::query(
        r#"
        SELECT 
            m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
            m.storage_key, COALESCE(m.external_url, lb.file_url) as external_url,
            m.order_index, m.visibility, m.is_active, 
            m.created_at, m.updated_at,
            m.class_id, c.name as class_name,
            m.teacher_id,
            COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
            COALESCE(s.name, lb.subject_name, 'Umum') as subject_name,
            m.start_page, m.end_page
        FROM learning_materials m
        LEFT JOIN classes c ON c.id = m.class_id
        LEFT JOIN subjects s ON s.id = m.subject_id
        LEFT JOIN teachers t ON t.id = m.teacher_id
        LEFT JOIN users ut ON ut.id = t.user_id
        LEFT JOIN users uc ON uc.id = m.created_by
        LEFT JOIN library_books lb ON lb.id = m.library_book_id
        WHERE m.id = $1 AND m.tenant_id = $2 AND m.deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let row = row_opt.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::LearningMaterialNotFound,
                format!("Learning material {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let is_completed = if is_student {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM student_material_completions 
                WHERE material_id = $1 AND student_id IN (SELECT id FROM students WHERE user_id = $2)
            ) as "exists!""#,
            id,
            actor_id
        )
        .fetch_one(&ctx.pool)
        .await
        .ok()
    } else {
        None
    };

    let completed_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*)::bigint as "count!" FROM student_material_completions WHERE material_id = $1"#,
        id
    )
    .fetch_one(&ctx.pool)
    .await
    .ok();

    let resp = LearningMaterialResponse {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        lesson_id: row.get("lesson_id"),
        material_type: row.get("material_type"),
        title: row.get("title"),
        description: row.get("description"),
        storage_key: row.get("storage_key"),
        external_url: row.get("external_url"),
        order_index: row.get("order_index"),
        visibility: row.get("visibility"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        is_completed,
        completed_count,
        teacher_name: row.get("teacher_name"),
        teacher_id: row.get("teacher_id"),
        class_name: row.get("class_name"),
        class_id: row.get("class_id"),
        subject_name: row.get("subject_name"),
        start_page: row.get("start_page"),
        end_page: row.get("end_page"),
    };

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct MaterialCompletionToggleResponse {
    pub material_id: Uuid,
    pub is_completed: bool,
    pub message: String,
}

async fn toggle_complete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<MaterialCompletionToggleResponse>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthTokenExpired,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Find student_id
    let student_id = sqlx::query_scalar!(
        r#"SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1"#,
        actor_id,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?
    .ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Only registered students can mark materials as completed".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Check if already completed
    let existing = sqlx::query_scalar!(
        r#"SELECT id FROM student_material_completions WHERE student_id = $1 AND material_id = $2"#,
        student_id,
        id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let is_completed = if existing.is_some() {
        // Toggle OFF (unmark completed)
        sqlx::query!(
            r#"DELETE FROM student_material_completions WHERE student_id = $1 AND material_id = $2"#,
            student_id,
            id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;
        false
    } else {
        // Toggle ON (mark completed)
        sqlx::query!(
            r#"INSERT INTO student_material_completions (tenant_id, student_id, material_id) VALUES ($1, $2, $3)"#,
            req_ctx.tenant_id,
            student_id,
            id
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;
        true
    };

    Ok(Json(ApiResponse::success(
        MaterialCompletionToggleResponse {
            material_id: id,
            is_completed,
            message: if is_completed {
                "Materi pembelajaran berhasil ditandai selesai".to_string()
            } else {
                "Status selesai materi pembelajaran dibatalkan".to_string()
            },
        },
        req_ctx.request_id,
    )))
}

async fn get_completed_materials(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<Uuid>>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let rows = sqlx::query_scalar!(
        r#"
        SELECT smc.material_id 
        FROM student_material_completions smc
        JOIN students s ON s.id = smc.student_id
        WHERE s.user_id = $1 AND smc.tenant_id = $2
        "#,
        actor_id,
        req_ctx.tenant_id
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    Ok(Json(ApiResponse::success(rows, req_ctx.request_id)))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct MaterialStudentCompletionDto {
    pub student_id: Uuid,
    pub student_name: String,
    pub nisn: Option<String>,
    pub gender: Option<String>,
    pub class_name: Option<String>,
    pub is_completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_page: Option<i32>,
    pub last_read_at: Option<DateTime<Utc>>,
}

async fn get_material_completions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<MaterialStudentCompletionDto>>>, ApiError> {
    // 1. Ambil material untuk mengetahui tenant_id dan class_id rombel
    let mat_row = sqlx::query(
        r#"
        SELECT tenant_id, class_id FROM learning_materials
        WHERE id = $1 AND deleted_at IS NULL
        "#
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?
    .ok_or_else(|| ApiError::new(school_core::common::error::ApplicationError::NotFound(
        school_core::common::error_code::ErrorCode::LearningMaterialNotFound,
        format!("Learning material {} not found", id)
    ), &req_ctx.request_id))?;

    let mat_class_id: Option<Uuid> = mat_row.get("class_id");

    // 2. Query siswa:
    // Jika materi terafiliasi dengan kelas tertentu, tampilkan semua siswa rombel tersebut dengan status selesai / progres bacanya.
    // Jika tidak terafiliasi dengan kelas spesifik, tampilkan seluruh siswa yang telah menyelesaikan atau membaca materi ini.
    let rows = if let Some(class_id) = mat_class_id {
        sqlx::query(
            r#"
            SELECT 
                s.id as student_id,
                s.full_name as student_name,
                s.nisn,
                s.gender,
                c.name as class_name,
                (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) as is_completed,
                COALESCE(smc.completed_at, rp.updated_at) as completed_at,
                rp.current_page,
                rp.last_read_at
            FROM students s
            JOIN enrollments en ON en.student_id = s.id AND en.class_id = $2
            JOIN classes c ON c.id = en.class_id
            LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = $1
            LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = $1
            WHERE s.tenant_id = $3
            ORDER BY is_completed DESC, s.full_name ASC
            "#
        )
        .bind(id)
        .bind(class_id)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT 
                s.id as student_id,
                s.full_name as student_name,
                s.nisn,
                s.gender,
                c.name as class_name,
                (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) as is_completed,
                COALESCE(smc.completed_at, rp.updated_at) as completed_at,
                rp.current_page,
                rp.last_read_at
            FROM students s
            LEFT JOIN enrollments en ON en.student_id = s.id
            LEFT JOIN classes c ON c.id = en.class_id
            LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = $1
            LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = $1
            WHERE s.tenant_id = $2 AND (smc.id IS NOT NULL OR rp.id IS NOT NULL)
            ORDER BY is_completed DESC, s.full_name ASC
            "#
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
    }
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let dtos: Vec<MaterialStudentCompletionDto> = rows.into_iter().map(|r| MaterialStudentCompletionDto {
        student_id: r.get("student_id"),
        student_name: r.get("student_name"),
        nisn: r.get("nisn"),
        gender: r.get("gender"),
        class_name: r.get("class_name"),
        is_completed: r.get("is_completed"),
        completed_at: r.get("completed_at"),
        current_page: r.get("current_page"),
        last_read_at: r.get("last_read_at"),
    }).collect();

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala sekolah") || n.contains("operator")
        })
    }).unwrap_or(false);

    let is_teacher = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "guru" || n.contains("teacher")
        })
    }).unwrap_or(false);

    if is_teacher && !is_admin {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (created_by = $3 OR teacher_id IN (SELECT id FROM teachers WHERE user_id = $3))
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !owns {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Anda tidak memiliki izin mengubah materi guru lain".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let command = UpdateLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
        title: payload.title,
        description: payload.description,
        storage_key: payload.storage_key,
        external_url: payload.external_url,
        visibility: payload.visibility,
    };

    let material = ctx
        .update_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LearningMaterialResponse::from(material),
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
    require_permission(&req_ctx.actor, Permission::LearningMaterialDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala sekolah") || n.contains("operator")
        })
    }).unwrap_or(false);

    let is_teacher = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n == "guru" || n.contains("teacher")
        })
    }).unwrap_or(false);

    if is_teacher && !is_admin {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (created_by = $3 OR teacher_id IN (SELECT id FROM teachers WHERE user_id = $3))
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !owns {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Anda tidak memiliki izin menghapus materi guru lain".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let command = DeleteLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

/// Upload a PDF or image file for use as learning material.
/// Returns a JSON with `url` field pointing to the uploaded file.
async fn upload_file(
    State(_ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru" || r.name == "Teacher")).unwrap_or(false);
    if !is_teacher {
        use crate::middleware::require_permission;
        use school_core::permission::domain::permission_registry::Permission;
        require_permission(&req_ctx.actor, Permission::LearningMaterialCreate).map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;
    }

    // Create uploads directory if it doesn't exist
    let uploads_dir = std::path::PathBuf::from("uploads");
    tokio::fs::create_dir_all(&uploads_dir).await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(
                format!("Failed to create uploads directory: {e}"),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut file_url: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(
                format!("Multipart error: {e}"),
            ),
            &req_ctx.request_id,
        )
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            let original_filename = field.file_name()
                .unwrap_or("upload")
                .to_string();
            let content_type = field.content_type().unwrap_or("").to_string();
            let mut ext = std::path::Path::new(&original_filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ext.is_empty() {
                ext = match content_type.as_str() {
                    "application/pdf" => "pdf".to_string(),
                    "image/jpeg" | "image/jpg" => "jpg".to_string(),
                    "image/png" => "png".to_string(),
                    "image/webp" => "webp".to_string(),
                    "image/gif" => "gif".to_string(),
                    _ => "bin".to_string(),
                };
            }

            // Only allow safe file types
            let allowed = ["pdf", "jpg", "jpeg", "png", "gif", "webp"];
            if !allowed.contains(&ext.as_str()) {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            format!("Tipe file .{ext} tidak diizinkan. Hanya PDF dan gambar yang diperbolehkan."),
                        ),
                    ),
                    &req_ctx.request_id,
                ));
            }

            let bytes = field.bytes().await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(
                        format!("Failed to read file bytes: {e}"),
                    ),
                    &req_ctx.request_id,
                )
            })?;

            // Generate unique filename
            let unique_name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
            let file_path = uploads_dir.join(&unique_name);

            tokio::fs::write(&file_path, &bytes).await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(
                        format!("Failed to save file: {e}"),
                    ),
                    &req_ctx.request_id,
                )
            })?;

            // Construct URL (served by backend static file handler or Nginx)
            file_url = Some(format!("/uploads/{unique_name}"));
        }
    }

    match file_url {
        Some(url) => Ok(Json(ApiResponse::success(
            serde_json::json!({ "url": url }),
            req_ctx.request_id,
        ))),
        None => Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Tidak ada file yang diunggah dalam request.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        )),
    }
}
