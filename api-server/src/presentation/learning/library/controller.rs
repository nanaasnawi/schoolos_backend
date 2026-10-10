use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, DomainError};
use school_core::common::error_code::ErrorCode;

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LibraryBookDto {
    pub id: Uuid,
    pub title: String,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub grade_level_id: Option<Uuid>,
    pub grade_level_name: Option<String>,
    pub class_level: Option<i32>,
    pub total_pages: i32,
    pub cover_url: Option<String>,
    pub file_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListBooksQuery {
    pub subject_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub class_level: Option<i32>,
    pub search: Option<String>,
    pub recommendations: Option<bool>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AssignReadingMaterialRequest {
    pub book_id: Uuid,
    pub title: String,
    pub instructions: Option<String>,
    pub class_id: Uuid,
    pub subject_id: Option<Uuid>,
    #[serde(default)]
    pub teacher_id: Option<Uuid>,
    pub start_page: i32,
    pub end_page: i32,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateReadingProgressRequest {
    pub material_id: Uuid,
    pub current_page: i32,
    pub is_completed: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ReadingAnalyticsResponse {
    pub material_id: Uuid,
    pub total_assigned: i64,
    pub total_started: i64,
    pub total_completed: i64,
    pub not_started: i64,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ReadingProgressResponse {
    pub student_id: Uuid,
    pub material_id: Uuid,
    pub current_page: i32,
    pub is_completed: bool,
    pub last_read_at: DateTime<Utc>,
}

pub fn library_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/books", get(list_books))
        .route("/recommendations", get(get_reading_recommendations))
        .route("/books/{id}", get(get_book_detail))
        .route("/assign", post(assign_reading_material))
        .route("/progress", post(update_reading_progress))
        .route("/materials/{id}/analytics", get(get_reading_analytics))
}

async fn get_reading_recommendations(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<ApiResponse<Vec<LibraryBookDto>>>, ApiError> {
    let mut modified_query = query;
    modified_query.recommendations = Some(true);
    list_books(State(ctx), req_ctx, Query(modified_query)).await
}

async fn list_books(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<ApiResponse<Vec<LibraryBookDto>>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_admin_or_staff = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin")
                    || n.contains("staff")
                    || n.contains("operator")
                    || n.contains("kepala")
                    || n.contains("bendahara")
            })
        })
        .unwrap_or(false);

    let is_teacher = !is_admin_or_staff
        && req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
                })
            })
            .unwrap_or(false);

    let is_student = !is_admin_or_staff
        && !is_teacher
        && req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("siswa") || n.contains("student") || n.contains("murid")
                })
            })
            .unwrap_or(false);

    // 1. Resolve specific class level if class_id is passed
    let specific_class_level: Option<i32> = if let Some(cid) = query.class_id {
        sqlx::query_scalar::<_, Option<i32>>(
            r#"
            SELECT COALESCE(
                gl.level,
                NULLIF(substring(c.tingkat from '[0-9]+'), '')::int,
                NULLIF(substring(c.name from '[0-9]+'), '')::int
            )
            FROM classes c
            LEFT JOIN grade_levels gl ON gl.id = c.grade_level_id
            WHERE c.id = $1 AND c.tenant_id = $2
            "#,
        )
        .bind(cid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .flatten()
    } else {
        None
    };

    // 2. Resolve student enrolled class level if caller is student or recommendations requested
    let is_recommendations = query.recommendations.unwrap_or(false) || is_student;
    let student_class_level: Option<i32> = if specific_class_level.is_none() && is_recommendations {
        if let Some(aid) = actor_id {
            sqlx::query_scalar::<_, Option<i32>>(
                r#"
                SELECT COALESCE(
                    gl.level,
                    NULLIF(substring(c.tingkat from '[0-9]+'), '')::int,
                    NULLIF(substring(c.name from '[0-9]+'), '')::int
                )
                FROM students s
                JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
                JOIN classes c ON c.id = en.class_id
                LEFT JOIN grade_levels gl ON gl.id = c.grade_level_id
                WHERE s.user_id = $1 AND s.tenant_id = $2
                LIMIT 1
                "#,
            )
            .bind(aid)
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    // 3. Resolve teacher taught class levels if caller is teacher and no specific class passed
    let teacher_id = if let Some(aid) = actor_id {
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

    let teacher_taught_levels: Option<Vec<i32>> =
        if is_teacher && specific_class_level.is_none() && query.class_level.is_none() {
            if let Some(tid) = teacher_id {
                let levels: Vec<i32> = sqlx::query_scalar::<_, Option<i32>>(
                    r#"
                    SELECT DISTINCT COALESCE(
                        gl.level,
                        NULLIF(substring(c.tingkat from '[0-9]+'), '')::int,
                        NULLIF(substring(c.name from '[0-9]+'), '')::int
                    )
                    FROM classes c
                    LEFT JOIN grade_levels gl ON gl.id = c.grade_level_id
                    WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
                      AND (
                        c.homeroom_teacher_id = $2
                        OR EXISTS (
                          SELECT 1 FROM class_schedules cs 
                          WHERE cs.class_id = c.id AND cs.teacher_id = $2 AND cs.tenant_id = $1
                        )
                      )
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(tid)
                .fetch_all(&ctx.pool)
                .await
                .ok()
                .map(|v| v.into_iter().flatten().collect())
                .unwrap_or_default();

                if !levels.is_empty() {
                    Some(levels)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Determine target exact level: specific class_id takes top priority, then student class, then query.class_level
    let exact_class_level: Option<i32> = specific_class_level
        .or(student_class_level)
        .or(query.class_level);

    let rows = sqlx::query(
        r#"
        SELECT 
            b.id, b.title, b.author, b.publisher, b.subject_id,
            COALESCE(sub.name, b.subject_name) as subject_name,
            b.grade_level_id,
            COALESCE(gl.name, CASE WHEN b.class_level IS NOT NULL THEN 'Kelas ' || b.class_level ELSE NULL END) as grade_level_name,
            b.class_level,
            b.total_pages, b.cover_url, b.file_url
        FROM library_books b
        LEFT JOIN subjects sub ON sub.id = b.subject_id AND sub.tenant_id = $1
        LEFT JOIN grade_levels gl ON gl.id = b.grade_level_id AND gl.tenant_id = $1
        WHERE (b.tenant_id = $1 OR b.tenant_id IS NULL)
          AND (
            $2::uuid IS NULL 
            OR b.subject_id = $2
            OR (
                b.canonical_subject IS NOT NULL 
                AND EXISTS (
                    SELECT 1 FROM subjects s 
                    WHERE s.id = $2 AND s.tenant_id = $1 
                      AND (
                        (b.canonical_subject = 'MATEMATIKA' AND (s.code LIKE '4010%' OR s.name ILIKE '%Matematika%'))
                        OR (b.canonical_subject = 'BAHASA_INDONESIA' AND (s.code LIKE '3001%' OR s.name ILIKE '%Indonesia%'))
                        OR (b.canonical_subject = 'BAHASA_INGGRIS' AND (s.code LIKE '3002%' OR s.name ILIKE '%Inggris%'))
                        OR (b.canonical_subject = 'FISIKA' AND (s.name ILIKE '%Fisika%'))
                        OR (b.canonical_subject = 'BIOLOGI' AND (s.name ILIKE '%Biologi%'))
                        OR (b.canonical_subject = 'KIMIA' AND (s.name ILIKE '%Kimia%'))
                        OR (b.canonical_subject = 'INFORMATIKA' AND (s.code LIKE '700%' OR s.name ILIKE '%Informatika%' OR s.name ILIKE '%Komputer%'))
                        OR (b.canonical_subject = 'PENDIDIKAN_PANCASILA' AND (s.code LIKE '200%' OR s.name ILIKE '%Pancasila%' OR s.name ILIKE '%PPKn%'))
                        OR (b.canonical_subject = 'PJOK' AND (s.code LIKE '500%' OR s.name ILIKE '%Jasmani%' OR s.name ILIKE '%PJOK%'))
                        OR (b.canonical_subject = 'PENDIDIKAN_AGAMA_ISLAM' AND (s.code LIKE '100%' OR s.name ILIKE '%Agama Islam%'))
                        OR (b.canonical_subject = 'SEJARAH' AND (s.name ILIKE '%Sejarah%'))
                        OR (b.canonical_subject = 'GEOGRAFI' AND (s.name ILIKE '%Geografi%'))
                        OR (b.canonical_subject = 'EKONOMI' AND (s.name ILIKE '%Ekonomi%'))
                        OR (b.canonical_subject = 'SOSIOLOGI' AND (s.name ILIKE '%Sosiologi%'))
                        OR (b.canonical_subject = 'SENI_BUDAYA' AND (s.code LIKE '843%' OR s.name ILIKE '%Seni%'))
                        OR (b.subject_name IS NOT NULL AND (s.name ILIKE '%' || b.subject_name || '%' OR b.subject_name ILIKE '%' || s.name || '%'))
                      )
                )
            )
          )
          AND (
            $3::uuid IS NULL 
            OR b.grade_level_id = $3
            OR (
                b.class_level IS NOT NULL 
                AND EXISTS (
                    SELECT 1 FROM grade_levels g 
                    WHERE g.id = $3 AND g.tenant_id = $1 AND g.level = b.class_level
                )
            )
          )
          AND (
            $4::int IS NULL 
            OR b.class_level = $4
            OR (
                b.grade_level_id IS NOT NULL 
                AND EXISTS (
                    SELECT 1 FROM grade_levels g
                    WHERE g.id = b.grade_level_id AND g.level = $4
                )
            )
          )
          AND (
            $5::int[] IS NULL 
            OR b.class_level = ANY($5)
            OR (
                b.grade_level_id IS NOT NULL 
                AND EXISTS (
                    SELECT 1 FROM grade_levels g
                    WHERE g.id = b.grade_level_id AND g.level = ANY($5)
                )
            )
          )
          AND ($6::text IS NULL OR b.title ILIKE '%' || $6 || '%' OR b.author ILIKE '%' || $6 || '%')
          AND (
            $7::boolean IS FALSE
            OR (b.title NOT ILIKE '%Panduan Guru%' AND b.title NOT ILIKE '%Buku Guru%')
          )
        ORDER BY 
          CASE WHEN $7::boolean THEN (b.file_url IS NOT NULL) END DESC NULLS LAST,
          CASE WHEN $7::boolean THEN (b.canonical_subject IS NOT NULL) END DESC NULLS LAST,
          (b.tenant_id IS NOT NULL) DESC,
          b.title ASC
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(query.subject_id)
    .bind(query.grade_level_id)
    .bind(exact_class_level)
    .bind(teacher_taught_levels.as_deref())
    .bind(query.search.as_deref().map(|s| s.trim()))
    .bind(is_recommendations)
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

    let items = rows
        .into_iter()
        .map(|r| LibraryBookDto {
            id: r.get("id"),
            title: r.get("title"),
            author: r.get("author"),
            publisher: r.get("publisher"),
            subject_id: r.get("subject_id"),
            subject_name: r.get("subject_name"),
            grade_level_id: r.get("grade_level_id"),
            grade_level_name: r.get("grade_level_name"),
            class_level: r.get("class_level"),
            total_pages: r.get("total_pages"),
            cover_url: r.get("cover_url"),
            file_url: r.get("file_url"),
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_book_detail(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LibraryBookDto>>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT 
            b.id, b.title, b.author, b.publisher, b.subject_id,
            COALESCE(sub.name, b.subject_name) as subject_name,
            b.grade_level_id,
            COALESCE(gl.name, CASE WHEN b.class_level IS NOT NULL THEN 'Kelas ' || b.class_level ELSE NULL END) as grade_level_name,
            b.class_level,
            b.total_pages, b.cover_url, b.file_url
        FROM library_books b
        LEFT JOIN subjects sub ON sub.id = b.subject_id AND sub.tenant_id = $2
        LEFT JOIN grade_levels gl ON gl.id = b.grade_level_id AND gl.tenant_id = $2
        WHERE b.id = $1 AND (b.tenant_id = $2 OR b.tenant_id IS NULL)
        LIMIT 1
        "#
    )
    .bind(id)
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
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::NotFound(
                ErrorCode::ResourceNotFound,
                format!("Buku perpustakaan {} tidak ditemukan", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let dto = LibraryBookDto {
        id: row.get("id"),
        title: row.get("title"),
        author: row.get("author"),
        publisher: row.get("publisher"),
        subject_id: row.get("subject_id"),
        subject_name: row.get("subject_name"),
        grade_level_id: row.get("grade_level_id"),
        grade_level_name: row.get("grade_level_name"),
        class_level: row.get("class_level"),
        total_pages: row.get("total_pages"),
        cover_url: row.get("cover_url"),
        file_url: row.get("file_url"),
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

async fn assign_reading_material(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<AssignReadingMaterialRequest>,
) -> Result<Json<ApiResponse<Uuid>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialCreate).map_err(|_| {
        ApiError::new(
            ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Hanya guru atau staf yang dapat menugaskan materi bacaan".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
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
    let teacher_id = payload.teacher_id.or(resolved_actor_teacher_id);

    // 1. Fetch target class & resolve its grade level
    let target_class = sqlx::query(
        r#"
        SELECT c.name, COALESCE(
            gl.level,
            NULLIF(substring(c.tingkat from '[0-9]+'), '')::int,
            NULLIF(substring(c.name from '[0-9]+'), '')::int
        ) as target_level,
        c.homeroom_teacher_id
        FROM classes c
        LEFT JOIN grade_levels gl ON gl.id = c.grade_level_id
        WHERE c.id = $1 AND c.tenant_id = $2
        "#
    )
    .bind(payload.class_id)
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
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::NotFound(
                ErrorCode::ResourceNotFound,
                "Kelas target tidak ditemukan".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let class_name: String = target_class.get("name");
    let target_level: Option<i32> = target_class.get("target_level");

    // 2. Fetch book details
    let book_row = sqlx::query(
        "SELECT title, class_level, file_url FROM library_books WHERE id = $1 AND (tenant_id = $2 OR tenant_id IS NULL)"
    )
    .bind(payload.book_id)
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
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::NotFound(
                ErrorCode::ResourceNotFound,
                format!("Buku SIBI {} tidak ditemukan", payload.book_id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let book_title: String = book_row.get("title");
    let book_class_level: Option<i32> = book_row.get("class_level");
    let book_file_url: Option<String> = book_row.get("file_url");

    // 3. STRICT CHECK: SIBI book must match class grade level to avoid assigning wrong grade books
    if let (Some(b_lvl), Some(c_lvl)) = (book_class_level, target_level) {
        if b_lvl != c_lvl {
            return Err(ApiError::new(
                ApplicationError::Domain(DomainError::Validation(format!(
                    "Buku SIBI \"{}\" (Kelas {}) tidak sesuai dengan kelas tujuan \"{}\" (Kelas {}). Mohon gunakan buku yang sesuai dengan kelas yang diampu.",
                    book_title, b_lvl, class_name, c_lvl
                ))),
                &req_ctx.request_id,
            ));
        }
    }

    // 4. Verify teacher is authorized to teach this class (homeroom or scheduled)
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("staff") || n.contains("operator") || n.contains("kepala")
        })
    }).unwrap_or(false);

    if !is_admin {
        if let Some(tid) = teacher_id {
            let homeroom_id: Option<Uuid> = target_class.get("homeroom_teacher_id");
            let is_homeroom = homeroom_id == Some(tid);
            let is_scheduled = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM class_schedules 
                    WHERE class_id = $1 AND teacher_id = $2 AND tenant_id = $3
                )
                "#
            )
            .bind(payload.class_id)
            .bind(tid)
            .bind(req_ctx.tenant_id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap_or(false);

            if !is_homeroom && !is_scheduled {
                return Err(ApiError::new(
                    ApplicationError::Unauthorized(
                        ErrorCode::AuthPermissionDenied,
                        format!("Anda tidak mengampu kelas \"{}\". Penugasan materi buku SIBI hanya dapat dilakukan pada kelas yang diampu.", class_name),
                    ),
                    &req_ctx.request_id,
                ));
            }
        }
    }

    let material_id = Uuid::new_v4();
    let description = format!(
        "Materi Bacaan Buku (Halaman {} — {}). {}",
        payload.start_page,
        payload.end_page,
        payload.instructions.as_deref().unwrap_or("")
    );

    sqlx::query(
        r#"
        INSERT INTO learning_materials (
            id, tenant_id, class_id, subject_id, teacher_id, created_by,
            material_type, title, description, external_url, is_active, source_type,
            library_book_id, start_page, end_page, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'document', $7, $8, $9, true, 'LIBRARY', $10, $11, $12, NOW(), NOW())
        "#
    )
    .bind(material_id)
    .bind(req_ctx.tenant_id)
    .bind(payload.class_id)
    .bind(payload.subject_id)
    .bind(teacher_id)
    .bind(actor_id)
    .bind(&payload.title)
    .bind(&description)
    .bind(&book_file_url)
    .bind(payload.book_id)
    .bind(payload.start_page)
    .bind(payload.end_page)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Fetch teacher & class names for notifications
    let teacher_name = if let Some(tid) = teacher_id {
        sqlx::query_scalar::<_, String>(
            "SELECT u.full_name FROM teachers t JOIN users u ON u.id = t.user_id WHERE t.id = $1",
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

    let class_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM classes WHERE id = $1 AND tenant_id = $2",
    )
    .bind(payload.class_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Kelas".to_string());

    let notif_title = format!("📚 Materi Baru: {}", payload.title);
    let notif_body = format!(
        "{} telah menugaskan materi bacaan baru untuk kelas {}. Buka dan pelajari sekarang!",
        teacher_name, class_name
    );

    // Insert in-app notifications for all active enrolled students in this class
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
        WHERE en.class_id = $4 AND (en.status = 'Active' OR en.status = 'ACTIVE')
        "#
    )
    .bind(&notif_title)
    .bind(&notif_body)
    .bind(material_id)
    .bind(payload.class_id)
    .execute(&ctx.pool)
    .await;

    // Trigger high-priority FCM push notification (wakes up lockscreen & alerts students)
    crate::infrastructure::fcm::trigger_fcm_push_notification(
        notif_title,
        notif_body,
        "Materi Pembelajaran".to_string(),
        material_id,
    );

    Ok(Json(ApiResponse::success(material_id, req_ctx.request_id)))
}

async fn update_reading_progress(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<UpdateReadingProgressRequest>,
) -> Result<Json<ApiResponse<ReadingProgressResponse>>, ApiError> {
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

    let row = sqlx::query(
        r#"
        INSERT INTO reading_progress (id, tenant_id, student_id, material_id, current_page, is_completed, last_read_at, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW(), NOW(), NOW())
        ON CONFLICT (student_id, material_id) DO UPDATE
        SET current_page = EXCLUDED.current_page,
            is_completed = EXCLUDED.is_completed,
            last_read_at = NOW(),
            updated_at = NOW()
        RETURNING student_id, material_id, current_page, is_completed, last_read_at
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(student_id)
    .bind(payload.material_id)
    .bind(payload.current_page)
    .bind(payload.is_completed)
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

    let resp = ReadingProgressResponse {
        student_id: row.get("student_id"),
        material_id: row.get("material_id"),
        current_page: row.get("current_page"),
        is_completed: row.get("is_completed"),
        last_read_at: row.get("last_read_at"),
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_reading_analytics(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(material_id): Path<Uuid>,
) -> Result<Json<ApiResponse<ReadingAnalyticsResponse>>, ApiError> {
    let material_row =
        sqlx::query("SELECT class_id FROM learning_materials WHERE id = $1 AND tenant_id = $2")
            .bind(material_id)
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
            })?
            .ok_or_else(|| {
                ApiError::new(
                    ApplicationError::NotFound(
                        ErrorCode::ResourceNotFound,
                        "Materi pembelajaran tidak ditemukan".to_string(),
                    ),
                    &req_ctx.request_id,
                )
            })?;

    let class_id: Option<Uuid> = material_row.get("class_id");

    let total_assigned: i64 = if let Some(cid) = class_id {
        sqlx::query(
            "SELECT COUNT(*)::bigint as total FROM enrollments WHERE class_id = $1 AND tenant_id = $2 AND (status = 'Active' OR status = 'ACTIVE')"
        )
        .bind(cid)
        .bind(req_ctx.tenant_id)
        .fetch_one(&ctx.pool)
        .await
        .map(|r| r.get("total"))
        .unwrap_or(0)
    } else {
        0
    };

    let progress_counts = sqlx::query(
        r#"
        SELECT 
            COUNT(*)::bigint as total_started,
            COUNT(CASE WHEN is_completed = true THEN 1 END)::bigint as total_completed
        FROM reading_progress
        WHERE material_id = $1 AND tenant_id = $2
        "#,
    )
    .bind(material_id)
    .bind(req_ctx.tenant_id)
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

    let total_started: i64 = progress_counts.get("total_started");
    let total_completed: i64 = progress_counts.get("total_completed");
    let not_started = (total_assigned - total_started).max(0);

    let resp = ReadingAnalyticsResponse {
        material_id,
        total_assigned,
        total_started,
        total_completed,
        not_started,
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}
