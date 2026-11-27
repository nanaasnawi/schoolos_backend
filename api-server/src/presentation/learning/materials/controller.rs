use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct MaterialFilterQuery {
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub lesson_id: Option<Uuid>,
    pub date: Option<String>,
    pub session_id: Option<Uuid>,
}

use super::dto::{
    create_learning_material_request::CreateLearningMaterialRequest,
    learning_material_response::{determine_material_format, LearningMaterialResponse},
    update_learning_material_request::UpdateLearningMaterialRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::authorization::domain::actor::Actor;
use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;
use school_core::learning::application::learning_material::{
    create_learning_material::CreateLearningMaterialCommand,
    delete_learning_material::DeleteLearningMaterialCommand,
    update_learning_material::UpdateLearningMaterialCommand,
};

fn actor_has_role_keyword(actor: &Option<Actor>, keywords: &[&str]) -> bool {
    actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                keywords.iter().any(|kw| n.contains(kw))
            })
        })
        .unwrap_or(false)
}

fn actor_has_admin_material_role(actor: &Option<Actor>) -> bool {
    actor_has_role_keyword(
        actor,
        &[
            "admin",
            "kepala",
            "principal",
            "operator",
            "staff",
            "staf",
            "super",
            "tu",
        ],
    )
}

fn actor_has_teacher_material_role(actor: &Option<Actor>) -> bool {
    actor_has_role_keyword(actor, &["guru", "teacher", "pengajar"])
}

fn map_material_row(
    r: &sqlx::postgres::PgRow,
    is_completed: Option<bool>,
    progress_percent: Option<i32>,
    completed_count: Option<i64>,
) -> LearningMaterialResponse {
    let m_type: String = r.get("material_type");
    let ext_url: Option<String> = r.get("external_url");
    let desc: Option<String> = r.get("description");
    let src_type: Option<String> = r.try_get::<Option<String>, _>("source_type").ok().flatten();
    let lib_id: Option<Uuid> = r.try_get::<Option<Uuid>, _>("library_book_id").ok().flatten();
    let lib_cover: Option<String> = r
        .try_get::<Option<String>, _>("library_cover_url")
        .ok()
        .flatten();
    let fmt = determine_material_format(
        &m_type,
        ext_url.as_deref(),
        desc.as_deref(),
        src_type.as_deref(),
        lib_id,
    );
    let stored_thumb = r
        .try_get::<Option<String>, _>("thumbnail_url")
        .ok()
        .flatten()
        .and_then(|s| crate::infrastructure::thumbnail_resolver::normalize_image_url(&s));
    let resolved_thumb = stored_thumb
        .or_else(|| {
            crate::infrastructure::thumbnail_resolver::resolve_deterministic_material_thumbnail(
                &m_type,
                ext_url.as_deref(),
                desc.as_deref(),
                src_type.as_deref(),
                lib_id,
            )
        })
        .or_else(|| {
            lib_cover
                .as_deref()
                .and_then(crate::infrastructure::thumbnail_resolver::normalize_image_url)
        });

    LearningMaterialResponse {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        lesson_id: r.get("lesson_id"),
        material_type: m_type,
        format: fmt.clone(),
        content_type: fmt,
        title: r.get("title"),
        description: desc,
        storage_key: r.get("storage_key"),
        external_url: ext_url,
        thumbnail_url: resolved_thumb,
        order_index: r.get("order_index"),
        visibility: r.get("visibility"),
        is_active: r.get("is_active"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        is_completed,
        progress_percent,
        completed_count,
        teacher_name: r.get("teacher_name"),
        teacher_id: r.get("teacher_id"),
        class_name: r.get("class_name"),
        class_id: r.get("class_id"),
        subject_name: r.get("subject_name"),
        start_page: r.get("start_page"),
        end_page: r.get("end_page"),
        session_id: r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
        release_at: r
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at")
            .ok()
            .flatten(),
    }
}

pub fn material_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route(
            "/upload",
            post(upload_file).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route("/analytics/reading-summary", get(get_reading_summary))
        .route("/completed", get(get_completed_materials))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/toggle-complete", post(toggle_complete))
        .route("/{id}/completions", get(get_material_completions))
        .route("/{id}/progress", post(update_material_reading_progress))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    let is_admin = actor_has_admin_material_role(&req_ctx.actor);
    let is_teacher = !is_admin && actor_has_teacher_material_role(&req_ctx.actor);

    if !is_teacher && !is_admin {
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

    let is_scheduled = payload.release_at.is_some_and(|r| r > chrono::Utc::now())
        || payload.visibility.to_lowercase() == "draft"
        || payload.visibility.to_lowercase() == "scheduled";
    let initial_visibility = if is_scheduled {
        "draft".to_string()
    } else {
        payload.visibility.clone()
    };

    let clean_external_url = payload.external_url.and_then(|u| {
        let t = u.trim();
        if t.is_empty() {
            None
        } else if t.starts_with("uploads/") {
            Some(format!("/{t}"))
        } else {
            Some(t.to_string())
        }
    });

    let clean_storage_key = payload.storage_key.and_then(|k| {
        let trimmed = k.trim();
        if trimmed == "Modul Digital"
            || trimmed == "Infografis Interaktif"
            || trimmed == "Artikel Teks"
            || trimmed == "YouTube"
            || trimmed == "-"
            || trimmed.is_empty()
        {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let command = CreateLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        material_type: payload.material_type,
        title: payload.title.clone(),
        description: payload.description.clone(),
        storage_key: clean_storage_key,
        external_url: clean_external_url,
        order_index: payload.order_index,
        visibility: initial_visibility.clone(),
    };

    let material = ctx
        .create_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM teachers WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1"#
        )
        .bind(aid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let validated_payload_teacher_id = if let Some(pid) = payload.teacher_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM teachers WHERE (id = $1 OR user_id = $1) AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1"#
        )
        .bind(pid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .or(Some(pid))
    } else {
        None
    };

    // Only strict teacher accounts default to themselves; principals/admins/staff assign the chosen payload.teacher_id.
    let teacher_id = if is_teacher && !is_admin {
        resolved_actor_teacher_id.or(validated_payload_teacher_id)
    } else {
        validated_payload_teacher_id.or(resolved_actor_teacher_id)
    };

    // Resolve target_class_id from UUID or class name string (e.g. "PAKET C10" or "Kelas 5")
    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#
                )
                .bind(req_ctx.tenant_id)
                .bind(cid_str.trim())
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
                    sqlx::query_scalar::<_, Uuid>(
                        r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#
                    )
                    .bind(req_ctx.tenant_id)
                    .bind(cand)
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

    // Fallback: Resolve subject_id from description, teacher profile, or title if not provided
    let target_subject_id: Option<Uuid> = if payload.subject_id.is_some() {
        payload.subject_id
    } else {
        let mut resolved_sub: Option<Uuid> = None;
        if let Some(ref desc) = payload.description {
            let parts: Vec<&str> = desc.split(" • ").collect();
            if !parts.is_empty() && !parts[0].trim().starts_with('[') && !parts[0].trim().starts_with('{') {
                let subj_name = parts[0].trim();
                resolved_sub = sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name ILIKE $2) LIMIT 1"#
                )
                .bind(req_ctx.tenant_id)
                .bind(subj_name)
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();
            }
        }
        if resolved_sub.is_none() {
            if let Some(tid) = teacher_id {
                resolved_sub = sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT s.id FROM subjects s 
                       JOIN teachers t ON lower(trim(t.subject)) = lower(trim(s.name)) 
                       WHERE t.id = $1 AND s.tenant_id = $2 AND s.deleted_at IS NULL LIMIT 1"#
                )
                .bind(tid)
                .bind(req_ctx.tenant_id)
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();
            }
        }
        if resolved_sub.is_none() {
            resolved_sub = sqlx::query_scalar::<_, Uuid>(
                r#"SELECT id FROM subjects 
                   WHERE tenant_id = $1 AND deleted_at IS NULL 
                     AND ($2 ILIKE '%' || name || '%' OR name ILIKE '%' || $2 || '%') 
                   ORDER BY length(name) DESC LIMIT 1"#
            )
            .bind(req_ctx.tenant_id)
            .bind(&material.title)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
        }
        resolved_sub
    };

    let final_release_at = if !is_scheduled {
        payload.release_at.or_else(|| Some(chrono::Utc::now()))
    } else {
        payload.release_at
    };

    if let Err(e) = sqlx::query(
        r#"
        UPDATE learning_materials 
        SET class_id = $1, 
            subject_id = COALESCE($2, subject_id), 
            teacher_id = $3, 
            created_by = $4,
            session_id = $5,
            release_at = $6,
            visibility = $7,
            notification_dispatched = $8
        WHERE id = $9
        "#
    )
    .bind(target_class_id)
    .bind(target_subject_id)
    .bind(teacher_id)
    .bind(actor_id)
    .bind(payload.session_id)
    .bind(final_release_at)
    .bind(&initial_visibility)
    .bind(!is_scheduled)
    .bind(material.id)
    .execute(&ctx.pool)
    .await {
        tracing::error!("Failed to update learning_materials on create: {:?}", e);
    }

    // Auto-archive previous duplicate materials with identical title for this class to prevent duplicates
    if !is_scheduled {
        if let Some(cid) = target_class_id {
            let _ = sqlx::query(
                r#"
                UPDATE learning_materials 
                SET visibility = 'archived', is_active = false, updated_at = NOW() 
                WHERE tenant_id = $1 
                  AND class_id = $2 
                  AND LOWER(TRIM(title)) = LOWER(TRIM($3))
                  AND id != $4 
                  AND is_active = true
                "#
            )
            .bind(req_ctx.tenant_id)
            .bind(cid)
            .bind(&material.title)
            .bind(material.id)
            .execute(&ctx.pool)
            .await;
        }
    }

    // Trigger in-app and FCM push notifications to enrolled students only if published immediately
    if !is_scheduled {
        let teacher_name = if let Some(tid) = teacher_id {
            sqlx::query_scalar::<_, String>(
                "SELECT COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(u.full_name), ''), 'Guru Pengampu') FROM teachers t LEFT JOIN users u ON u.id = t.user_id WHERE t.id = $1",
            )
            .bind(tid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "Guru Pengampu".to_string())
        } else if let Some(aid) = actor_id {
            sqlx::query_scalar::<_, String>("SELECT full_name FROM users WHERE id = $1")
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
            "SELECT name FROM classes WHERE id = $1 AND tenant_id = $2",
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
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
            SELECT 
                gen_random_uuid(),
                s.tenant_id,
                s.user_id,
                $1,
                $2,
                'LEARNING_MATERIAL',
                'in_app',
                'material',
                $3,
                FALSE,
                NOW()
            FROM students s
            JOIN enrollments en ON en.student_id = s.id
            JOIN classes en_c ON en_c.id = en.class_id
            JOIN classes target_c ON target_c.id = $4
                 AND en_c.id = target_c.id
            WHERE en_c.tenant_id = target_c.tenant_id AND (en.status ILIKE 'active')
            "#
        )
        .bind(&notif_title)
        .bind(&notif_body)
        .bind(material.id)
        .bind(cid)
        .execute(&ctx.pool)
        .await;

        let sibling_cids = vec![cid];

        crate::infrastructure::fcm::trigger_fcm_push_targeted(
            req_ctx.tenant_id,
            notif_title.clone(),
            notif_body,
            crate::infrastructure::fcm::FcmCategory::Material,
            material.id,
            crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
        );
    } else {
        let notif_body = format!(
            "{} telah menambahkan modul pembelajaran baru: {}. Pelajari sekarang!",
            teacher_name, material.title
        );

        let _ = sqlx::query(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
            SELECT 
                gen_random_uuid(),
                s.tenant_id,
                s.user_id,
                $1,
                $2,
                'LEARNING_MATERIAL',
                'in_app',
                'material',
                $3,
                FALSE,
                NOW()
            FROM students s
            WHERE s.tenant_id = $4
            "#
        )
        .bind(&notif_title)
        .bind(&notif_body)
        .bind(material.id)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;

        crate::infrastructure::fcm::trigger_fcm_push_targeted(
            req_ctx.tenant_id,
            notif_title,
            notif_body,
            crate::infrastructure::fcm::FcmCategory::Material,
            material.id,
            crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
        );
    }
    }

    // Resolve or schedule 1x thumbnail generation ONLY when material publishes immediately.
    if !is_scheduled
        && (initial_visibility.eq_ignore_ascii_case("published")
            || initial_visibility.eq_ignore_ascii_case("active"))
    {
        let _ = crate::infrastructure::thumbnail_resolver::ensure_material_thumbnail_on_publish(
            ctx.pool.clone(),
            material.id,
        )
        .await;
    }

    let full_created = sqlx::query(
        r#"
        SELECT 
            m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
            m.storage_key,
            COALESCE(
                NULLIF(TRIM(m.external_url), ''),
                CASE
                    WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                    WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                    ELSE NULL
                END,
                lb.file_url
            ) as external_url,
            m.thumbnail_url, m.source_type, m.library_book_id, lb.cover_url as library_cover_url,
            m.order_index, m.visibility, m.is_active, 
            m.created_at, m.updated_at,
            m.class_id, c.name as class_name,
            m.teacher_id,
            COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name,
            COALESCE(s.name, lb.subject_name, t.subject, 'Umum') as subject_name,
            m.start_page, m.end_page,
            m.session_id, m.release_at
        FROM learning_materials m
        LEFT JOIN classes c ON c.id = m.class_id
        LEFT JOIN subjects s ON s.id = m.subject_id
        LEFT JOIN teachers t ON t.id = m.teacher_id
        LEFT JOIN users ut ON ut.id = t.user_id
        LEFT JOIN users uc ON uc.id = m.created_by
        LEFT JOIN library_books lb ON lb.id = m.library_book_id
        WHERE m.id = $1 AND m.tenant_id = $2
        "#,
    )
    .bind(material.id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let resp = match full_created {
        Some(r) => map_material_row(&r, None, None, Some(0)),
        None => LearningMaterialResponse::from(material),
    };

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    Query(filter): Query<MaterialFilterQuery>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<LearningMaterialResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin")
                    || n.contains("kepala")
                    || n.contains("operator")
                    || n.contains("tu")
                    || n.contains("staff")
                    || n.contains("staf")
            })
        })
        .unwrap_or(false);

    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
            &ctx.pool,
            req_ctx.tenant_id,
            aid,
        )
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let is_teacher = !is_admin && (
        req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
                })
            })
            .unwrap_or(false)
            || resolved_actor_teacher_id.is_some()
    );

    let is_student = !is_admin
        && !is_teacher
        && req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n == "siswa" || n.contains("student") || n == "murid"
                })
            })
            .unwrap_or(false);

    let items: Vec<LearningMaterialResponse> = if is_teacher {
        // Teacher strictly sees only their own materials:
        // Either created by them, or assigned to their teacher record (even if uploaded by admin)
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key,
            COALESCE(
                NULLIF(TRIM(m.external_url), ''),
                CASE
                    WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                    WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                    ELSE NULL
                END,
                lb.file_url
            ) as external_url,
                m.order_index, 
                CASE WHEN m.release_at > NOW() THEN 'draft' WHEN m.release_at <= NOW() AND (m.visibility = 'draft' OR m.visibility IS NULL) THEN 'published' ELSE COALESCE(m.visibility, 'published') END as visibility, 
                m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, t.subject, 'Umum') as subject_name,
                m.start_page, m.end_page,
                m.thumbnail_url, m.source_type, m.library_book_id, lb.cover_url as library_cover_url,
                m.session_id, m.release_at,
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
                  OR ($3::uuid IS NOT NULL AND m.teacher_id = $3)
                  OR m.teacher_id = $2
                  OR m.teacher_id IN (
                      SELECT id FROM teachers 
                      WHERE (user_id = $2 OR id = $2 
                             OR lower(trim(full_name)) = (SELECT lower(trim(full_name)) FROM users WHERE id = $2)
                             OR (email IS NOT NULL AND email != '-' AND lower(trim(email)) = (SELECT lower(trim(email)) FROM users WHERE id = $2)))
                        AND tenant_id = $1 AND deleted_at IS NULL
                  )
              )
              AND ($4::uuid IS NULL OR m.class_id = $4)
              AND ($5::uuid IS NULL OR m.subject_id = $5)
              AND ($6::uuid IS NULL OR m.lesson_id = $6)
              AND ($7::text IS NULL OR (COALESCE(m.release_at, m.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $7::date)
              AND ($8::uuid IS NULL OR m.session_id = $8)
            ORDER BY m.updated_at DESC, m.created_at DESC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(resolved_actor_teacher_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.lesson_id)
        .bind(filter.date)
        .bind(filter.session_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter()
            .map(|r| {
                let completed: Option<i64> = r.get("completed_count");
                map_material_row(&r, None, None, completed)
            })
            .collect()
    } else if is_student && !is_admin {
        // Student sees materials for their active enrolled classes or general materials
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key,
            COALESCE(
                NULLIF(TRIM(m.external_url), ''),
                CASE
                    WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                    WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                    ELSE NULL
                END,
                lb.file_url
            ) as external_url,
                m.order_index, 
                CASE WHEN m.release_at > NOW() THEN 'draft' WHEN m.release_at <= NOW() AND (m.visibility = 'draft' OR m.visibility IS NULL) THEN 'published' ELSE COALESCE(m.visibility, 'published') END as visibility, 
                m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, t.subject, 'Umum') as subject_name,
                m.start_page, m.end_page,
                m.thumbnail_url, m.source_type, m.library_book_id, lb.cover_url as library_cover_url,
                m.session_id, m.release_at,
                (smc.id IS NOT NULL) as is_completed,
                CASE
                    WHEN smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true THEN 100
                    WHEN rp.id IS NOT NULL THEN LEAST(95, GREATEST(10, ROUND((rp.current_page::float8 / GREATEST(1, COALESCE(m.end_page, 20) - COALESCE(m.start_page, 1) + 1)::float8) * 100.0))::int)
                    ELSE NULL
                END as progress_percent
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN subjects s ON s.id = m.subject_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = m.created_by
            LEFT JOIN library_books lb ON lb.id = m.library_book_id
            LEFT JOIN student_material_completions smc 
                ON smc.material_id = m.id 
                AND smc.student_id IN (SELECT id FROM students WHERE user_id = $2 OR id = $2)
            LEFT JOIN reading_progress rp
                ON rp.material_id = m.id
                AND rp.student_id IN (SELECT id FROM students WHERE user_id = $2 OR id = $2)
            WHERE m.tenant_id = $1 
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND (m.release_at IS NULL OR m.release_at <= NOW())
              AND (
                  m.visibility = 'published' 
                  OR m.visibility = 'active'
                  OR (m.release_at IS NOT NULL AND m.release_at <= NOW())
              )
              AND (
                  m.class_id IS NULL OR
                  m.class_id IN (
                      SELECT en.class_id FROM students s JOIN enrollments en ON en.student_id = s.id WHERE (s.user_id = $2 OR s.id = $2) AND (en.status ILIKE 'active' OR en.status IS NULL)
                  )
              )
              AND ($3::uuid IS NULL OR m.class_id = $3)
              AND ($4::uuid IS NULL OR m.subject_id = $4)
              AND ($5::uuid IS NULL OR m.lesson_id = $5)
              AND ($6::text IS NULL OR (COALESCE(m.release_at, m.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $6::date)
              AND ($7::uuid IS NULL OR m.session_id = $7)
            ORDER BY m.updated_at DESC, m.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.lesson_id)
        .bind(filter.date)
        .bind(filter.session_id)
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
            .map(|r| {
                let is_comp: bool = r.get("is_completed");
                let prog = r.try_get::<Option<i32>, _>("progress_percent").ok().flatten();
                map_material_row(&r, Some(is_comp), prog, None)
            })
            .collect()
    } else if is_admin {
        // Super Admin / Kepala Sekolah / Staf sees all materials in tenant
        let rows = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.lesson_id, m.material_type, m.title, m.description, 
                m.storage_key,
            COALESCE(
                NULLIF(TRIM(m.external_url), ''),
                CASE
                    WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                    WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                    ELSE NULL
                END,
                lb.file_url
            ) as external_url,
                m.order_index, 
                CASE WHEN m.release_at > NOW() THEN 'draft' WHEN m.release_at <= NOW() AND (m.visibility = 'draft' OR m.visibility IS NULL) THEN 'published' ELSE COALESCE(m.visibility, 'published') END as visibility, 
                m.is_active, 
                m.created_at, m.updated_at,
                m.class_id, c.name as class_name,
                m.teacher_id,
                COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name,
                COALESCE(s.name, lb.subject_name, t.subject, 'Umum') as subject_name,
                m.start_page, m.end_page,
                m.thumbnail_url, m.source_type, m.library_book_id, lb.cover_url as library_cover_url,
                m.session_id, m.release_at,
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
              AND ($2::uuid IS NULL OR m.teacher_id = $2 OR m.created_by = $2)
              AND ($3::uuid IS NULL OR m.class_id = $3)
              AND ($4::uuid IS NULL OR m.subject_id = $4)
              AND ($5::uuid IS NULL OR m.lesson_id = $5)
              AND ($6::text IS NULL OR (COALESCE(m.release_at, m.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $6::date)
              AND ($7::uuid IS NULL OR m.session_id = $7)
            ORDER BY m.updated_at DESC, m.created_at DESC
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(filter.teacher_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.lesson_id)
        .bind(filter.date)
        .bind(filter.session_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.into_iter()
            .map(|r| {
                let completed: Option<i64> = r.get("completed_count");
                map_material_row(&r, None, None, completed)
            })
            .collect()
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
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin") || n.contains("kepala sekolah") || n.contains("kepala") || n.contains("operator") || n.contains("staff") || n.contains("staf")
            })
        })
        .unwrap_or(false);

    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
            &ctx.pool,
            req_ctx.tenant_id,
            aid,
        )
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let is_teacher = !is_admin && (
        req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
                })
            })
            .unwrap_or(false)
            || resolved_actor_teacher_id.is_some()
    );

    let is_student = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n == "siswa" || n.contains("student")
            })
        })
        .unwrap_or(false);

    if is_teacher {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (
                      created_by = $3 
                      OR ($4::uuid IS NOT NULL AND teacher_id = $4)
                      OR teacher_id = $3
                      OR teacher_id IN (
                          SELECT id FROM teachers 
                          WHERE (user_id = $3 OR id = $3 
                                 OR lower(trim(full_name)) = (SELECT lower(trim(full_name)) FROM users WHERE id = $3)
                                 OR (email IS NOT NULL AND email != '-' AND lower(trim(email)) = (SELECT lower(trim(email)) FROM users WHERE id = $3)))
                            AND tenant_id = $2 AND deleted_at IS NULL
                      )
                  )
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(resolved_actor_teacher_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        // Teachers within the same tenant can read published materials
        if !owns {
            tracing::debug!("Teacher reading shared material from another teacher in same tenant");
        }
    } else if is_student && !is_admin {
        let can_access = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials m
                WHERE m.id = $1 AND m.tenant_id = $2 AND m.deleted_at IS NULL
                  AND (
                      m.class_id IS NULL OR
                      m.class_id IN (
                          SELECT en.class_id FROM students s JOIN enrollments en ON en.student_id = s.id WHERE (s.user_id = $3 OR s.id = $3) AND (en.status ILIKE 'active' OR en.status IS NULL)
                      )
                      OR EXISTS (
                          SELECT 1 FROM students s WHERE (s.user_id = $3 OR s.id = $3) AND s.tenant_id = $2
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
                school_core::common::error::ApplicationError::Forbidden(
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
            m.storage_key,
            COALESCE(
                NULLIF(TRIM(m.external_url), ''),
                CASE
                    WHEN m.storage_key LIKE '/uploads/%' OR m.storage_key LIKE 'http%' THEN m.storage_key
                    WHEN m.storage_key LIKE 'uploads/%' THEN '/' || m.storage_key
                    ELSE NULL
                END,
                lb.file_url
            ) as external_url,
            m.order_index, m.visibility, m.is_active, 
            m.created_at, m.updated_at,
            m.class_id, c.name as class_name,
            m.teacher_id,
            COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name,
            COALESCE(s.name, lb.subject_name, t.subject, 'Umum') as subject_name,
            m.start_page, m.end_page,
            m.thumbnail_url, m.source_type, m.library_book_id, lb.cover_url as library_cover_url,
            m.session_id, m.release_at
        FROM learning_materials m
        LEFT JOIN classes c ON c.id = m.class_id
        LEFT JOIN subjects s ON s.id = m.subject_id
        LEFT JOIN teachers t ON t.id = m.teacher_id
        LEFT JOIN users ut ON ut.id = t.user_id
        LEFT JOIN users uc ON uc.id = m.created_by
        LEFT JOIN library_books lb ON lb.id = m.library_book_id
        WHERE m.id = $1 AND m.tenant_id = $2 AND m.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
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
        if let Some(uid) = actor_id {
            let _ = crate::presentation::learning::attendance_automation::record_online_material_presence(
                &ctx.pool,
                req_ctx.tenant_id,
                uid,
                id,
            )
            .await;
        }

        sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM student_material_completions 
                WHERE material_id = $1 AND student_id IN (SELECT id FROM students WHERE user_id = $2)
            )"#,
        )
        .bind(id)
        .bind(actor_id)
        .fetch_one(&ctx.pool)
        .await
        .ok()
    } else {
        None
    };

    let completed_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)::bigint FROM student_material_completions WHERE material_id = $1"#,
    )
    .bind(id)
    .fetch_one(&ctx.pool)
    .await
    .ok();

    let resp = map_material_row(&row, is_completed, None, completed_count);

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
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

    // Find student_id (resilient to user_id or student direct id)
    let student_id = match sqlx::query_scalar::<_, Uuid>(
        r#"SELECT id FROM students WHERE (user_id = $1 OR id = $1) AND (tenant_id = $2 OR $2::uuid IS NULL) LIMIT 1"#,
    )
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await {
        Ok(Some(sid)) => sid,
        _ => {
            sqlx::query_scalar::<_, Uuid>(
                r#"SELECT id FROM students WHERE user_id = $1 OR id = $1 LIMIT 1"#,
            )
            .bind(actor_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Unauthorized(
                        school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                        "Only registered students can mark materials as completed".to_string(),
                    ),
                    &req_ctx.request_id,
                )
            })?
        }
    };

    // Check if already completed
    let existing: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM student_material_completions WHERE student_id = $1 AND material_id = $2"#
    )
    .bind(student_id)
    .bind(id)
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

    let is_completed = if existing.is_some() {
        // Status is PERMANENTLY LOCKED once completed (anti-cheating: tidak boleh batal)
        true
    } else {
        // Toggle ON (mark completed)
        sqlx::query(
            r#"INSERT INTO student_material_completions (tenant_id, student_id, material_id) VALUES ($1, $2, $3)"#
        )
        .bind(req_ctx.tenant_id)
        .bind(student_id)
        .bind(id)
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        let total_pages: i32 = sqlx::query_scalar(
            r#"SELECT (COALESCE(end_page, 15) - COALESCE(start_page, 1) + 1)::integer FROM learning_materials WHERE id = $1"#
        )
        .bind(id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(15);

        let _ = sqlx::query(
            r#"
            INSERT INTO reading_progress (id, tenant_id, student_id, material_id, current_page, is_completed, last_read_at, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, true, NOW(), NOW(), NOW())
            ON CONFLICT (student_id, material_id) DO UPDATE
            SET current_page = EXCLUDED.current_page,
                is_completed = true,
                last_read_at = NOW(),
                updated_at = NOW()
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(student_id)
        .bind(id)
        .bind(total_pages)
        .execute(&ctx.pool)
        .await;

        let _ = crate::presentation::learning::attendance_automation::record_online_material_presence(
            &ctx.pool,
            req_ctx.tenant_id,
            req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default(),
            id,
        )
        .await;

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

#[derive(Debug, serde::Deserialize)]
pub struct MaterialCompletionsQuery {
    pub class_id: Option<Uuid>,
}

async fn get_material_completions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Query(query): Query<MaterialCompletionsQuery>,
) -> Result<Json<ApiResponse<Vec<MaterialStudentCompletionDto>>>, ApiError> {
    // 1. Ambil material untuk mengetahui tenant_id, class_id rombel, subject_id, dan teacher_id
    let mat_row = sqlx::query(
        r#"
        SELECT tenant_id, class_id, subject_id, teacher_id, title, description FROM learning_materials
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::LearningMaterialNotFound,
                format!("Learning material {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query_class_id: Option<Uuid> = query.class_id;
    let mat_class_id: Option<Uuid> = mat_row.get("class_id");
    let mat_subject_id: Option<Uuid> = mat_row.get("subject_id");
    let mat_teacher_id: Option<Uuid> = mat_row.get("teacher_id");
    let mat_title: String = mat_row.get("title");
    let mat_desc: Option<String> = mat_row.get("description");

    // Tentukan rombel/kelas target:
    // 1. Jika dikirim melalui query parameter ?class_id=...
    // 2. Jika tersimpan di kolom class_id materi
    // 3. Jika null, coba deteksi dari teks judul/deskripsi rombel
    // 4. Jika masih kosong, coba ambil rombel dari jadwal (class_schedules) guru & mapel tersebut
    let mut target_class_ids: Vec<Uuid> = Vec::new();

    if let Some(cid) = query_class_id {
        target_class_ids.push(cid);
    } else if let Some(cid) = mat_class_id {
        target_class_ids.push(cid);
    } else {
        let text_search = format!("{} {}", mat_title, mat_desc.as_deref().unwrap_or(""));
        let matched_classes: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM classes 
            WHERE tenant_id = $1 AND deleted_at IS NULL AND (
                $2 ILIKE ('%' || name || '%')
                OR ($2 ILIKE '%Kelas X%' AND (name = 'PAKET C 10' OR name ILIKE '%10%'))
                OR ($2 ILIKE '%Kelas XI%' AND (name ILIKE '%11%'))
                OR ($2 ILIKE '%Kelas XII%' AND (name ILIKE '%12%'))
                OR ($2 ILIKE '%Kelas VII%' AND (name = 'PAKET B 7' OR name ILIKE '%7%'))
                OR ($2 ILIKE '%Kelas VIII%' AND (name ILIKE '%8%'))
                OR ($2 ILIKE '%Kelas IX%' AND (name ILIKE '%9%'))
                OR ($2 ILIKE '%Kelas IV%' AND (name = 'PAKET A 4' OR name ILIKE '%4%'))
                OR ($2 ILIKE '%Kelas V%' AND (name = 'PAKET A 5' OR name ILIKE '%5%'))
                OR ($2 ILIKE '%Kelas VI%' AND (name = 'PAKET A 6' OR name ILIKE '%6%'))
            )
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(&text_search)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

        if !matched_classes.is_empty() {
            target_class_ids.extend(matched_classes);
        } else if let (Some(tid), Some(sid)) = (mat_teacher_id, mat_subject_id) {
            let sched_classes: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT DISTINCT class_id FROM class_schedules 
                WHERE teacher_id = $1 AND subject_id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(tid)
            .bind(sid)
            .fetch_all(&ctx.pool)
            .await
            .unwrap_or_default();
            if !sched_classes.is_empty() {
                target_class_ids.extend(sched_classes);
            }
        }
    }

    // 2. Query siswa:
    // Jika ada rombel target, tampilkan siswa rombel tersebut dengan status progres bacanya.
    // Jika tidak ada rombel target yang teridentifikasi, HANYA tampilkan siswa yang SUDAH berinteraksi / membaca / menyelesaikan materi ini.
    // JANGAN PERNAH menampilkan seluruh siswa 1 sekolah jika belum terasosiasi!
    let rows = if !target_class_ids.is_empty() {
        sqlx::query(
            r#"
            SELECT 
                student_id,
                student_name,
                nisn,
                gender,
                class_name,
                is_completed,
                completed_at,
                current_page,
                last_read_at
            FROM (
                SELECT DISTINCT ON (s.id)
                    s.id as student_id,
                    s.full_name as student_name,
                    s.nisn,
                    s.gender,
                    c.name as class_name,
                    (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) as is_completed,
                    COALESCE(smc.completed_at, rp.updated_at) as completed_at,
                    CASE 
                        WHEN (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) 
                            THEN COALESCE(rp.current_page, (COALESCE(m.end_page, 15) - COALESCE(m.start_page, 1) + 1)::integer)
                        ELSE rp.current_page
                    END as current_page,
                    COALESCE(rp.last_read_at, smc.completed_at) as last_read_at
                FROM students s
                JOIN enrollments en ON en.student_id = s.id AND en.class_id = ANY($2) AND (en.status = 'Active' OR en.status = 'active')
                JOIN classes c ON c.id = en.class_id
                CROSS JOIN (SELECT start_page, end_page FROM learning_materials WHERE id = $1) m
                LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = $1
                LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = $1
                WHERE s.tenant_id = $3 AND s.deleted_at IS NULL AND (s.status = 'active' OR s.status = 'Active' OR s.is_active = true)
                ORDER BY s.id, (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) DESC
            ) sub
            ORDER BY is_completed DESC, current_page DESC NULLS LAST, student_name ASC
            "#
        )
        .bind(id)
        .bind(&target_class_ids)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT 
                student_id,
                student_name,
                nisn,
                gender,
                class_name,
                is_completed,
                completed_at,
                current_page,
                last_read_at
            FROM (
                SELECT DISTINCT ON (s.id)
                    s.id as student_id,
                    s.full_name as student_name,
                    s.nisn,
                    s.gender,
                    c.name as class_name,
                    (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) as is_completed,
                    COALESCE(smc.completed_at, rp.updated_at) as completed_at,
                    CASE 
                        WHEN (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) 
                            THEN COALESCE(rp.current_page, (COALESCE(m.end_page, 15) - COALESCE(m.start_page, 1) + 1)::integer)
                        ELSE rp.current_page
                    END as current_page,
                    COALESCE(rp.last_read_at, smc.completed_at) as last_read_at
                FROM students s
                JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'active')
                JOIN classes c ON c.id = en.class_id
                CROSS JOIN (SELECT start_page, end_page FROM learning_materials WHERE id = $1) m
                LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = $1
                LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = $1
                WHERE s.tenant_id = $2 AND s.deleted_at IS NULL AND (s.status = 'active' OR s.status = 'Active' OR s.is_active = true)
                  AND (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true OR COALESCE(rp.current_page, 0) > 0)
                ORDER BY s.id, (smc.id IS NOT NULL OR COALESCE(rp.is_completed, false) = true) DESC
            ) sub
            ORDER BY is_completed DESC, current_page DESC NULLS LAST, student_name ASC
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

    let dtos: Vec<MaterialStudentCompletionDto> = rows
        .into_iter()
        .map(|r| MaterialStudentCompletionDto {
            student_id: r.get("student_id"),
            student_name: r.get("student_name"),
            nisn: r.get("nisn"),
            gender: r.get("gender"),
            class_name: r.get("class_name"),
            is_completed: r.get("is_completed"),
            completed_at: r.get("completed_at"),
            current_page: r.get("current_page"),
            last_read_at: r.get("last_read_at"),
        })
        .collect();

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateMaterialProgressRequest {
    pub current_page: i32,
    pub is_completed: Option<bool>,
}

async fn update_material_reading_progress(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateMaterialProgressRequest>,
) -> Result<Json<ApiResponse<MaterialStudentCompletionDto>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Autentikasi diperlukan".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id = crate::authorization_helpers::AuthorizationScope::resolve_student_id(
        &ctx.pool,
        req_ctx.tenant_id,
        actor_id,
    )
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Hanya siswa yang dapat memperbarui progres membaca".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let is_completed = payload.is_completed.unwrap_or(false);

    let row = sqlx::query(
        r#"
        INSERT INTO reading_progress (id, tenant_id, student_id, material_id, current_page, is_completed, last_read_at, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW(), NOW(), NOW())
        ON CONFLICT (student_id, material_id) DO UPDATE
        SET current_page = EXCLUDED.current_page,
            is_completed = EXCLUDED.is_completed,
            last_read_at = NOW(),
            updated_at = NOW()
        RETURNING current_page, is_completed, last_read_at
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(student_id)
    .bind(id)
    .bind(payload.current_page)
    .bind(is_completed)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    if is_completed {
        let _ = sqlx::query(
            r#"INSERT INTO student_material_completions (id, tenant_id, student_id, material_id, completed_at, created_at, updated_at) 
               VALUES (gen_random_uuid(), $1, $2, $3, NOW(), NOW(), NOW())
               ON CONFLICT (student_id, material_id) DO NOTHING"#
        )
        .bind(req_ctx.tenant_id)
        .bind(student_id)
        .bind(id)
        .execute(&ctx.pool)
        .await;
    }

    let _ = crate::presentation::learning::attendance_automation::record_online_material_presence(
        &ctx.pool,
        req_ctx.tenant_id,
        actor_id,
        id,
    )
    .await;

    let st = sqlx::query(
        r#"
        SELECT s.full_name, s.nisn, s.gender, c.name as class_name
        FROM students s
        LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'active')
        LEFT JOIN classes c ON c.id = en.class_id
        WHERE s.id = $1
        LIMIT 1
        "#
    )
    .bind(student_id)
    .fetch_optional(&ctx.pool)
    .await
    .unwrap_or(None);

    let (st_name, st_nisn, st_gender, st_class) = match st {
        Some(s) => (s.get("full_name"), s.get("nisn"), s.get("gender"), s.get("class_name")),
        None => ("Siswa".to_string(), None, None, None),
    };

    let row_is_completed: bool = row.get("is_completed");
    let row_current_page: i32 = row.get("current_page");
    let row_last_read_at: chrono::DateTime<chrono::Utc> = row.get("last_read_at");

    let dto = MaterialStudentCompletionDto {
        student_id,
        student_name: st_name,
        nisn: st_nisn,
        gender: st_gender,
        class_name: st_class,
        is_completed: row_is_completed,
        completed_at: if row_is_completed { Some(row_last_read_at) } else { None },
        current_page: Some(row_current_page),
        last_read_at: Some(row_last_read_at),
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
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
    let is_admin = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin") || n.contains("kepala sekolah") || n.contains("kepala") || n.contains("operator") || n.contains("staff") || n.contains("staf")
            })
        })
        .unwrap_or(false);

    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
            &ctx.pool,
            req_ctx.tenant_id,
            aid,
        )
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let is_teacher = !is_admin && (
        req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
                })
            })
            .unwrap_or(false)
            || resolved_actor_teacher_id.is_some()
    );

    if is_teacher && !is_admin {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (
                      created_by = $3 
                      OR ($4::uuid IS NOT NULL AND teacher_id = $4)
                      OR teacher_id = $3
                      OR teacher_id IN (
                          SELECT id FROM teachers 
                          WHERE (user_id = $3 OR id = $3 
                                 OR lower(trim(full_name)) = (SELECT lower(trim(full_name)) FROM users WHERE id = $3)
                                 OR (email IS NOT NULL AND email != '-' AND lower(trim(email)) = (SELECT lower(trim(email)) FROM users WHERE id = $3)))
                            AND tenant_id = $2 AND deleted_at IS NULL
                      )
                  )
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(resolved_actor_teacher_id)
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
        visibility: payload.visibility.clone(),
    };

    let material = ctx
        .update_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let target_vis = payload.visibility.as_deref();
    let is_activating = target_vis == Some("published") || target_vis == Some("active");

    if payload.start_page.is_some()
        || payload.end_page.is_some()
        || payload.release_at.is_some()
        || payload.class_id.is_some()
        || payload.subject_id.is_some()
        || payload.teacher_id.is_some()
        || payload.visibility.is_some()
    {
        let is_sched = payload.release_at.map(|r| r > chrono::Utc::now());
        let _ = sqlx::query(
            r#"UPDATE learning_materials 
               SET start_page = COALESCE($1, start_page), 
                   end_page = COALESCE($2, end_page),
                   release_at = CASE 
                       WHEN $9 = true THEN LEAST(COALESCE($3, release_at, NOW()), NOW())
                       ELSE COALESCE($3, release_at)
                   END,
                   class_id = COALESCE($4, class_id),
                   subject_id = COALESCE($5, subject_id),
                   teacher_id = COALESCE($10, teacher_id),
                   visibility = CASE 
                       WHEN $9 = true THEN 'published'
                       WHEN $6 = true THEN 'draft' 
                       WHEN $6 = false THEN 'published' 
                       ELSE COALESCE($7, visibility) 
                   END,
                   notification_dispatched = CASE 
                       WHEN $9 = true THEN true
                       WHEN $6 = true THEN false 
                       WHEN $6 = false THEN true 
                       ELSE notification_dispatched 
                   END,
                   is_active = true,
                   updated_at = NOW()
               WHERE id = $8"#
        )
        .bind(payload.start_page)
        .bind(payload.end_page)
        .bind(payload.release_at)
        .bind(payload.class_id)
        .bind(payload.subject_id)
        .bind(is_sched)
        .bind(payload.visibility.as_ref())
        .bind(id)
        .bind(is_activating)
        .bind(payload.teacher_id)
        .execute(&ctx.pool)
        .await;

        if is_activating {
            let notif_info = sqlx::query(
                r#"
                SELECT m.title, m.class_id, c.name as class_name,
                       COALESCE(NULLIF(TRIM(t.full_name), ''), NULLIF(TRIM(ut.full_name), ''), uc.full_name, 'Guru Pengampu') as teacher_name
                FROM learning_materials m
                LEFT JOIN classes c ON c.id = m.class_id
                LEFT JOIN teachers t ON t.id = m.teacher_id
                LEFT JOIN users ut ON ut.id = t.user_id
                LEFT JOIN users uc ON uc.id = m.created_by
                WHERE m.id = $1
                "#
            )
            .bind(id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            if let Some(n) = notif_info {
                use sqlx::Row;
                let title: String = n.get("title");
                let class_id: Option<Uuid> = n.try_get("class_id").ok().flatten();
                let class_name: Option<String> = n.try_get("class_name").ok().flatten();
                let teacher_name: String = n.get("teacher_name");

                let notif_title = format!("📚 Materi Baru: {}", title);
                let notif_body = if let Some(ref c_name) = class_name {
                    format!("{} telah menerbitkan modul pembelajaran baru untuk kelas {}. Pelajari sekarang!", teacher_name, c_name)
                } else {
                    format!("{} telah menerbitkan modul pembelajaran baru: {}. Buka aplikasi untuk membaca!", teacher_name, title)
                };

                if let Some(cid) = class_id {
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                        SELECT 
                            gen_random_uuid(), s.tenant_id, s.user_id, $1, $2, 'LEARNING_MATERIAL', 'in_app', 'material', $3, FALSE, NOW()
                        FROM students s
                        JOIN enrollments en ON en.student_id = s.id
                        WHERE en.class_id = $4 AND (en.status ILIKE 'active' OR en.status IS NULL)
                        "#
                    )
                    .bind(&notif_title)
                    .bind(&notif_body)
                    .bind(id)
                    .bind(cid)
                    .execute(&ctx.pool)
                    .await;

                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        req_ctx.tenant_id,
                        notif_title,
                        notif_body,
                        crate::infrastructure::fcm::FcmCategory::Material,
                        id,
                        crate::infrastructure::fcm::FcmTarget::Classes(vec![cid]),
                    );
                } else {
                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        req_ctx.tenant_id,
                        notif_title,
                        notif_body,
                        crate::infrastructure::fcm::FcmCategory::Material,
                        id,
                        crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
                    );
                }
            }
        }
    }

    let updated_thumb = crate::infrastructure::thumbnail_resolver::ensure_material_thumbnail_on_publish(
        ctx.pool.clone(),
        id,
    )
    .await;

    let mut resp = LearningMaterialResponse::from(material);
    resp.thumbnail_url = updated_thumb;
    if payload.start_page.is_some() {
        resp.start_page = payload.start_page;
    }
    if payload.end_page.is_some() {
        resp.end_page = payload.end_page;
    }
    if payload.release_at.is_some() {
        resp.release_at = payload.release_at;
        if let Some(is_s) = payload.release_at.map(|r| r > chrono::Utc::now()) {
            if is_s {
                resp.visibility = "draft".to_string();
            } else if payload.visibility.is_none() {
                resp.visibility = "published".to_string();
            }
        }
    }
    if payload.class_id.is_some() {
        resp.class_id = payload.class_id;
    }
    if payload.teacher_id.is_some() {
        resp.teacher_id = payload.teacher_id;
    }

    Ok(Json(ApiResponse::success(
        resp,
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
    let is_admin = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin") || n.contains("kepala sekolah") || n.contains("kepala") || n.contains("operator") || n.contains("staff") || n.contains("staf")
            })
        })
        .unwrap_or(false);

    let resolved_actor_teacher_id = crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
        &ctx.pool,
        req_ctx.tenant_id,
        actor_id,
    )
    .await
    .ok()
    .flatten();

    let is_teacher = !is_admin && (
        req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
                })
            })
            .unwrap_or(false)
            || resolved_actor_teacher_id.is_some()
    );

    if is_teacher && !is_admin {
        let owns = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM learning_materials
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                  AND (
                      created_by = $3 
                      OR ($4::uuid IS NOT NULL AND teacher_id = $4)
                      OR teacher_id = $3
                      OR teacher_id IN (
                          SELECT id FROM teachers 
                          WHERE (user_id = $3 OR id = $3 
                                 OR lower(trim(full_name)) = (SELECT lower(trim(full_name)) FROM users WHERE id = $3)
                                 OR (email IS NOT NULL AND email != '-' AND lower(trim(email)) = (SELECT lower(trim(email)) FROM users WHERE id = $3)))
                            AND tenant_id = $2 AND deleted_at IS NULL
                      )
                  )
            )"#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(resolved_actor_teacher_id)
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

/// Shared multipart file storage helper used by both learning materials (teachers/admin)
/// and student assignment submission uploads.
pub(crate) async fn store_uploaded_file(
    ctx: &ApplicationContext,
    req_ctx: &RequestContext,
    mut multipart: Multipart,
    allowed_exts: &[&str],
    allowed_types_label: &str,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    const MAX_MATERIAL_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

    // Create uploads directory if it doesn't exist
    let uploads_dir = std::path::PathBuf::from("uploads");
    tokio::fs::create_dir_all(&uploads_dir).await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(format!(
                "Failed to create uploads directory: {e}"
            )),
            &req_ctx.request_id,
        )
    })?;

    let mut file_url: Option<String> = None;
    let mut uploaded_key: Option<String> = None;
    let mut uploaded_size: usize = 0;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(format!("Multipart error: {e}")),
            &req_ctx.request_id,
        )
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            let original_filename = field.file_name().unwrap_or("upload").to_string();
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
                    "image/heic" => "heic".to_string(),
                    "image/heif" => "heif".to_string(),
                    "application/msword" => "doc".to_string(),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                        "docx".to_string()
                    }
                    "text/plain" => "txt".to_string(),
                    _ => "bin".to_string(),
                };
            }

            if !allowed_exts.contains(&ext.as_str()) {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(format!(
                            "Tipe file .{ext} tidak diizinkan. Hanya {allowed_types_label} yang diperbolehkan."
                        )),
                    ),
                    &req_ctx.request_id,
                ));
            }

            let bytes = field.bytes().await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(format!(
                        "Failed to read file bytes: {e}"
                    )),
                    &req_ctx.request_id,
                )
            })?;

            if bytes.is_empty() {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            "Berkas yang diunggah kosong (0 byte).".to_string(),
                        ),
                    ),
                    &req_ctx.request_id,
                ));
            }

            if bytes.len() > MAX_MATERIAL_UPLOAD_BYTES {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            "Ukuran berkas melebihi batas maksimal 25 MB.".to_string(),
                        ),
                    ),
                    &req_ctx.request_id,
                ));
            }

            if ext == "pdf" {
                let header_end = bytes.len().min(1024);
                let has_pdf_magic = bytes[..header_end].windows(5).any(|w| w == b"%PDF-");
                if !has_pdf_magic {
                    return Err(ApiError::new(
                        school_core::common::error::ApplicationError::Domain(
                            school_core::common::error::DomainError::Validation(
                                "Berkas bukan dokumen PDF yang valid.".to_string(),
                            ),
                        ),
                        &req_ctx.request_id,
                    ));
                }
            }

            // Generate unique filename
            let unique_name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
            let file_path = uploads_dir.join(&unique_name);

            tokio::fs::write(&file_path, &bytes).await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(format!(
                        "Failed to save file: {e}"
                    )),
                    &req_ctx.request_id,
                )
            })?;

            // Persist file in database uploaded_files table so container restarts/redeployments never lose user uploads
            let mime_val = guess_upload_mime(&unique_name);
            if let Err(err) = sqlx::query(
                r#"
                INSERT INTO uploaded_files (filename, mime_type, file_data)
                VALUES ($1, $2, $3)
                ON CONFLICT (filename) DO UPDATE
                SET file_data = EXCLUDED.file_data, mime_type = EXCLUDED.mime_type, created_at = NOW()
                "#,
            )
            .bind(&unique_name)
            .bind(mime_val)
            .bind(bytes.as_ref())
            .execute(&ctx.pool)
            .await
            {
                tracing::warn!("Failed persisting uploaded file {} to uploaded_files table: {:?}", unique_name, err);
            }

            // Construct URL (served by backend static file handler or Nginx)
            uploaded_key = Some(original_filename);
            uploaded_size = bytes.len();
            file_url = Some(format!("/uploads/{unique_name}"));
        }
    }

    match file_url {
        Some(url) => Ok(Json(ApiResponse::success(
            serde_json::json!({
                "url": url,
                "key": uploaded_key,
                "filename": uploaded_key,
                "size_bytes": uploaded_size,
            }),
            req_ctx.request_id.clone(),
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

/// Upload a PDF or image file for use as learning material.
/// Returns a JSON with `url` field pointing to the uploaded file.
async fn upload_file(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    multipart: Multipart,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let can_upload_role = actor_has_admin_material_role(&req_ctx.actor)
        || actor_has_teacher_material_role(&req_ctx.actor);
    if !can_upload_role {
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

    store_uploaded_file(
        &ctx,
        &req_ctx,
        multipart,
        &["pdf", "jpg", "jpeg", "png", "gif", "webp"],
        "PDF dan gambar",
    )
    .await
}

#[derive(Debug, serde::Serialize)]
pub struct ReadingSummaryResponse {
    pub materials: Vec<MaterialSummaryItem>,
    pub reading_rows: Vec<StudentReadingRow>,
    pub metrics: ReadingSummaryMetrics,
}

#[derive(Debug, serde::Serialize)]
pub struct MaterialSummaryItem {
    pub material_id: Uuid,
    pub title: String,
    pub material_type: String,
    pub class_name: String,
    pub class_id: Option<Uuid>,
    pub subject_name: String,
    pub start_page: i32,
    pub end_page: i32,
    pub total_pages: i32,
    pub total_assigned_students: i64,
    pub completed_students_count: i64,
    pub average_progress_percentage: i32,
}

#[derive(Debug, serde::Serialize)]
pub struct StudentReadingRow {
    pub student_id: Uuid,
    pub student_name: String,
    pub nisn: String,
    pub gender: String,
    pub class_id: Option<Uuid>,
    pub class_name: String,
    pub material_id: Uuid,
    pub material_title: String,
    pub subject_name: String,
    pub current_page: i32,
    pub total_pages: i32,
    pub completion_percentage: i32,
    pub is_completed: bool,
    pub last_read_at: Option<String>,
    pub completed_at: Option<String>,
    pub reading_status: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ReadingSummaryMetrics {
    pub total_materials: i64,
    pub total_readers: i64,
    pub completed_readers: i64,
    pub overall_completion_rate: i32,
    pub total_pages_read: i64,
}

pub async fn get_reading_summary(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<ReadingSummaryResponse>>, ApiError> {
    let is_staff_or_teacher = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin")
                    || n.contains("kepala")
                    || n.contains("operator")
                    || n.contains("staff")
                    || n.contains("staf")
                    || n.contains("tata usaha")
                    || n.contains("guru")
                    || n.contains("teacher")
                    || n.contains("tutor")
                    || n.contains("pengajar")
            })
        })
        .unwrap_or(false);

    if !is_staff_or_teacher {
        return Err(ApiError::new(
            ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Akses ditolak: Ringkasan aktivitas membaca siswa hanya tersedia untuk Pendidik dan Staf Sekolah.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    // Query 1: All active learning materials for the tenant with counts
    let material_rows = sqlx::query(
        r#"
        SELECT 
            m.id,
            m.title,
            m.material_type,
            m.class_id,
            COALESCE(c.name, 'Semua Rombel') AS class_name,
            COALESCE(s.name, 'Umum') AS subject_name,
            COALESCE(m.start_page, 1) AS start_page,
            COALESCE(m.end_page, 15) AS end_page,
            COALESCE((
                SELECT COUNT(DISTINCT e.student_id)
                FROM enrollments e
                WHERE e.tenant_id = m.tenant_id
                  AND (m.class_id IS NULL OR e.class_id = m.class_id)
                  AND UPPER(e.status) = 'ACTIVE'
            ), 0) AS total_assigned_students,
            COALESCE((
                SELECT COUNT(DISTINCT smc.student_id)
                FROM student_material_completions smc
                WHERE smc.material_id = m.id
                  AND smc.tenant_id = m.tenant_id
                  AND smc.is_completed = true
            ), 0) AS completed_students_count
        FROM learning_materials m
        LEFT JOIN classes c ON c.id = m.class_id AND c.tenant_id = m.tenant_id
        LEFT JOIN subjects s ON s.id = m.subject_id AND s.tenant_id = m.tenant_id
        WHERE m.tenant_id = $1
          AND m.deleted_at IS NULL
        ORDER BY m.created_at DESC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let mut materials = Vec::with_capacity(material_rows.len());
    for row in &material_rows {
        let m_id: Uuid = row.try_get("id").unwrap_or_default();
        let title: String = row.try_get("title").unwrap_or_default();
        let m_type: String = row.try_get("material_type").unwrap_or_else(|_| "document".to_string());
        let c_id: Option<Uuid> = row.try_get("class_id").unwrap_or(None);
        let c_name: String = row.try_get("class_name").unwrap_or_else(|_| "Semua Rombel".to_string());
        let s_name: String = row.try_get("subject_name").unwrap_or_else(|_| "Umum".to_string());
        let start_p: i32 = row.try_get("start_page").unwrap_or(1);
        let end_p: i32 = row.try_get("end_page").unwrap_or(15);
        let total_p = std::cmp::max(1, (end_p - start_p) + 1);
        let assigned: i64 = row.try_get("total_assigned_students").unwrap_or(0);
        let completed: i64 = row.try_get("completed_students_count").unwrap_or(0);
        let avg_pct = if assigned > 0 {
            std::cmp::min(100, ((completed * 100) / assigned) as i32)
        } else if completed > 0 {
            100
        } else {
            0
        };

        materials.push(MaterialSummaryItem {
            material_id: m_id,
            title,
            material_type: m_type,
            class_name: c_name,
            class_id: c_id,
            subject_name: s_name,
            start_page: start_p,
            end_page: end_p,
            total_pages: total_p,
            total_assigned_students: assigned,
            completed_students_count: completed,
            average_progress_percentage: avg_pct,
        });
    }

    // Query 2: All student reading progress records for materials of this tenant
    let reading_rows_data = sqlx::query(
        r#"
        SELECT 
            smc.student_id,
            st.full_name AS student_name,
            COALESCE(st.nisn, '-') AS nisn,
            COALESCE(st.gender, 'L') AS gender,
            e.class_id,
            COALESCE(c.name, 'Kelas Umum') AS class_name,
            smc.material_id,
            m.title AS material_title,
            COALESCE(sub.name, 'Umum') AS subject_name,
            COALESCE(smc.current_page, 0) AS current_page,
            COALESCE(m.end_page - m.start_page + 1, 15) AS total_pages,
            COALESCE(smc.completion_percentage, 0) AS completion_percentage,
            COALESCE(smc.is_completed, false) AS is_completed,
            smc.last_read_at,
            smc.completed_at
        FROM student_material_completions smc
        JOIN students st ON st.id = smc.student_id AND st.tenant_id = smc.tenant_id
        JOIN learning_materials m ON m.id = smc.material_id AND m.tenant_id = smc.tenant_id
        LEFT JOIN enrollments e ON e.student_id = st.id AND e.tenant_id = st.tenant_id AND UPPER(e.status) = 'ACTIVE'
        LEFT JOIN classes c ON c.id = e.class_id AND c.tenant_id = st.tenant_id
        LEFT JOIN subjects sub ON sub.id = m.subject_id AND sub.tenant_id = m.tenant_id
        WHERE smc.tenant_id = $1
          AND m.deleted_at IS NULL
          AND st.deleted_at IS NULL
        ORDER BY smc.last_read_at DESC NULLS LAST
        LIMIT 500
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let mut reading_rows = Vec::with_capacity(reading_rows_data.len());
    let mut total_pages_read: i64 = 0;
    let mut completed_readers_count: i64 = 0;

    for r in reading_rows_data {
        let st_id: Uuid = r.try_get("student_id").unwrap_or_default();
        let st_name: String = r.try_get("student_name").unwrap_or_default();
        let nisn: String = r.try_get("nisn").unwrap_or_else(|_| "-".to_string());
        let gender: String = r.try_get("gender").unwrap_or_else(|_| "L".to_string());
        let c_id: Option<Uuid> = r.try_get("class_id").unwrap_or(None);
        let c_name: String = r.try_get("class_name").unwrap_or_else(|_| "Kelas Umum".to_string());
        let m_id: Uuid = r.try_get("material_id").unwrap_or_default();
        let m_title: String = r.try_get("material_title").unwrap_or_default();
        let sub_name: String = r.try_get("subject_name").unwrap_or_else(|_| "Umum".to_string());
        let curr_p: i32 = r.try_get("current_page").unwrap_or(0);
        let tot_p: i32 = std::cmp::max(1, r.try_get("total_pages").unwrap_or(15));
        let comp_pct: i32 = r.try_get("completion_percentage").unwrap_or(0);
        let is_comp: bool = r.try_get("is_completed").unwrap_or(false);
        let last_read: Option<DateTime<Utc>> = r.try_get("last_read_at").unwrap_or(None);
        let comp_at: Option<DateTime<Utc>> = r.try_get("completed_at").unwrap_or(None);

        total_pages_read += curr_p as i64;
        if is_comp {
            completed_readers_count += 1;
        }

        let status_label = if is_comp {
            "Selesai Membaca".to_string()
        } else if curr_p > 0 {
            format!("Sedang Membaca (Halaman {})", curr_p)
        } else {
            "Belum Mulai".to_string()
        };

        reading_rows.push(StudentReadingRow {
            student_id: st_id,
            student_name: st_name,
            nisn,
            gender,
            class_id: c_id,
            class_name: c_name,
            material_id: m_id,
            material_title: m_title,
            subject_name: sub_name,
            current_page: curr_p,
            total_pages: tot_p,
            completion_percentage: comp_pct,
            is_completed: is_comp,
            last_read_at: last_read.map(|t| t.to_rfc3339()),
            completed_at: comp_at.map(|t| t.to_rfc3339()),
            reading_status: status_label,
        });
    }

    let total_readers = reading_rows.len() as i64;
    let overall_rate = if total_readers > 0 {
        std::cmp::min(100, ((completed_readers_count * 100) / total_readers) as i32)
    } else {
        0
    };

    let response = ReadingSummaryResponse {
        materials,
        reading_rows,
        metrics: ReadingSummaryMetrics {
            total_materials: material_rows.len() as i64,
            total_readers,
            completed_readers: completed_readers_count,
            overall_completion_rate: overall_rate,
            total_pages_read,
        },
    };

    Ok(Json(ApiResponse::success(response, req_ctx.request_id)))
}


fn build_uploaded_file_response(filename: &str, mime: &str, bytes: Vec<u8>) -> Response {
    let lower_mime = mime.trim().to_ascii_lowercase();
    let safe_mime = match lower_mime.as_str() {
        "text/html" | "application/xhtml+xml" | "application/javascript" | "text/javascript"
        | "application/x-javascript" | "text/xml" | "application/xml" => "application/octet-stream",
        _ => mime.trim(),
    };
    let out_bytes = sanitize_served_upload_bytes(safe_mime, bytes);
    let safe_filename = filename.replace('"', "");
    let is_svg = safe_mime.eq_ignore_ascii_case("image/svg+xml");
    let is_inline_safe = safe_mime.starts_with("image/")
        || safe_mime.starts_with("video/")
        || safe_mime == "application/pdf";
    let disposition = if is_inline_safe {
        format!("inline; filename=\"{safe_filename}\"")
    } else {
        format!("attachment; filename=\"{safe_filename}\"")
    };
    let mut builder = axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, safe_mime)
        .header(axum::http::header::CONTENT_DISPOSITION, disposition)
        .header(axum::http::header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(axum::http::header::ACCEPT_RANGES, "bytes")
        .header(
            axum::http::header::CACHE_CONTROL,
            "public, max-age=31536000, immutable",
        );
    if is_svg {
        builder = builder.header(
            axum::http::header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; sandbox",
        );
    }
    builder
        .body(axum::body::Body::from(out_bytes))
        .unwrap_or_else(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Error building response",
            )
                .into_response()
        })
}

pub async fn serve_uploaded_file(
    axum::extract::State(ctx): axum::extract::State<ApplicationContext>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    let clean = std::path::Path::new(&path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&path)
        .to_string();

    let disk_path = std::path::PathBuf::from("uploads").join(&clean);
    if disk_path.is_file() {
        if let Ok(bytes) = tokio::fs::read(&disk_path).await {
            let mime = guess_upload_mime(&clean);
            return build_uploaded_file_response(&clean, mime, bytes);
        }
    }

    // Fallback to PostgreSQL uploaded_files table
    let row = school_core::common::tenant_context::with_system_rls_bypass(
        sqlx::query("SELECT mime_type, file_data FROM uploaded_files WHERE filename = $1")
            .bind(&clean)
            .fetch_optional(&ctx.pool),
    )
    .await
    .ok()
    .flatten();

    if let Some(r) = row {
        let mime: String = r.get("mime_type");
        let raw_bytes: Vec<u8> = r.get("file_data");
        let sanitized = sanitize_served_upload_bytes(&mime, raw_bytes);

        // Write to local disk cache
        let _ = tokio::fs::create_dir_all("uploads").await;
        let _ = tokio::fs::write(&disk_path, &sanitized).await;

        return build_uploaded_file_response(&clean, &mime, sanitized);
    }

    (axum::http::StatusCode::NOT_FOUND, "File not found").into_response()
}

fn strip_tag_block_case_insensitive(input: &str, open_tag: &str, close_tag: &str) -> String {
    let mut out = input.to_string();
    loop {
        let lower = out.to_ascii_lowercase();
        let Some(start) = lower.find(open_tag) else {
            break;
        };
        if let Some(rel_end) = lower[start..].find(close_tag) {
            let end = start + rel_end + close_tag.len();
            out.replace_range(start..end, "");
        } else {
            out.truncate(start);
            break;
        }
    }
    out
}

/// Sanitizes SVG diagrams against embedded scripts/event handlers while preserving SVG 1.1 compatibility for AndroidSVG.
fn sanitize_served_upload_bytes(mime: &str, bytes: Vec<u8>) -> Vec<u8> {
    if mime.eq_ignore_ascii_case("image/svg+xml") {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            let mut cleaned = strip_tag_block_case_insensitive(text, "<script", "</script>");
            cleaned = strip_tag_block_case_insensitive(&cleaned, "<foreignobject", "</foreignobject>");
            // Strip inline event handlers and active URI schemes case-insensitively
            for dangerous in ["onload=", "onclick=", "onerror=", "onmouseover=", "onfocus=", "javascript:", "vbscript:"] {
                loop {
                    let lower = cleaned.to_ascii_lowercase();
                    let Some(pos) = lower.find(dangerous) else {
                        break;
                    };
                    cleaned.replace_range(pos..pos + dangerous.len(), "data-blocked=");
                }
            }
            if cleaned.contains("auto-start-reverse") {
                cleaned = cleaned.replace(r#"orient="auto-start-reverse""#, r#"orient="auto""#);
            }
            return cleaned.into_bytes();
        }
    }
    bytes
}

fn guess_upload_mime(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "svg" => "image/svg+xml",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "txt" => "text/plain; charset=utf-8",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}
