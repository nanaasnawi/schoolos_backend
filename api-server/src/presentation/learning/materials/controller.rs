use axum::{
    Json, Router,
    extract::{Path, State, Multipart, DefaultBodyLimit},
    routing::{get, post},
};
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
    get_learning_material::GetLearningMaterialQuery,
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
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
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
    let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru")).unwrap_or(false);
    let is_student = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Siswa")).unwrap_or(false);

    let items: Vec<LearningMaterialResponse> = if is_teacher {
        // Teacher sees only materials they created or assigned to them
        let rows = sqlx::query!(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, m.external_url, m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                (SELECT COUNT(*)::bigint FROM student_material_completions smc WHERE smc.material_id = m.id) as "completed_count!"
            FROM learning_materials m
            WHERE m.tenant_id = $1 
              AND m.deleted_at IS NULL
              AND (
                  m.created_by = $2 
                  OR m.teacher_id IN (SELECT id FROM teachers WHERE user_id = $2)
              )
            ORDER BY m.created_at DESC
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id,
            material_type: r.material_type,
            title: r.title,
            description: r.description,
            storage_key: r.storage_key,
            external_url: r.external_url,
            order_index: r.order_index,
            visibility: r.visibility,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_completed: None,
            completed_count: Some(r.completed_count),
        }).collect()
    } else if is_student {
        // Student sees materials ONLY for their active enrolled classes (Paket A / B / C strictly isolated)
        let rows = sqlx::query!(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, m.external_url, m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                (smc.id IS NOT NULL) as "is_completed!"
            FROM learning_materials m
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
            "#,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id,
            material_type: r.material_type,
            title: r.title,
            description: r.description,
            storage_key: r.storage_key,
            external_url: r.external_url,
            order_index: r.order_index,
            visibility: r.visibility,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_completed: Some(r.is_completed),
            completed_count: None,
        }).collect()
    } else {
        // Super Admin / Kepala Sekolah / Staf sees all materials in tenant
        let rows = sqlx::query!(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key, m.external_url, m.order_index, m.visibility, m.is_active, 
                m.created_at, m.updated_at,
                (SELECT COUNT(*)::bigint FROM student_material_completions smc WHERE smc.material_id = m.id) as "completed_count!"
            FROM learning_materials m
            WHERE m.tenant_id = $1 AND m.deleted_at IS NULL
            ORDER BY m.created_at DESC
            "#,
            req_ctx.tenant_id
        )
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter().map(|r| LearningMaterialResponse {
            id: r.id,
            tenant_id: r.tenant_id,
            lesson_id: r.lesson_id,
            material_type: r.material_type,
            title: r.title,
            description: r.description,
            storage_key: r.storage_key,
            external_url: r.external_url,
            order_index: r.order_index,
            visibility: r.visibility,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_completed: None,
            completed_count: Some(r.completed_count),
        }).collect()
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

    let query = GetLearningMaterialQuery {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
    };

    let material = ctx
        .get_learning_material
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_student = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Siswa")).unwrap_or(false);

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

    let mut resp = LearningMaterialResponse::from(material);
    resp.is_completed = is_completed;
    resp.completed_count = completed_count;

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
            let ext = std::path::Path::new(&original_filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin")
                .to_lowercase();

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
