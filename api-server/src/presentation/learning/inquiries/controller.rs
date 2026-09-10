use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InquiryThreadDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub student_class: String,
    pub teacher_id: Option<Uuid>,
    pub teacher_name: String,
    pub subject_name: String,
    pub inquiry_type: String,
    pub reference_title: String,
    pub reference_id: Option<String>,
    pub status: String,
    pub last_message_content: Option<String>,
    pub last_message_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InquiryMessageDto {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: String,
    pub sender_name: String,
    pub sender_role: String,
    pub content: String,
    pub is_from_teacher: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InquiryDetailDto {
    pub thread: InquiryThreadDto,
    pub messages: Vec<InquiryMessageDto>,
}

#[derive(Debug, Deserialize)]
pub struct ListInquiriesQuery {
    pub status: Option<String>,
    pub inquiry_type: Option<String>,
    pub search: Option<String>,
    pub student_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub teacher_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInquiryRequest {
    pub student_id: Option<Uuid>,
    pub student_name: Option<String>,
    pub student_class: Option<String>,
    pub teacher_id: Option<Uuid>,
    pub teacher_name: Option<String>,
    pub subject_name: Option<String>,
    pub inquiry_type: String,
    pub reference_title: String,
    pub reference_id: Option<String>,
    pub initial_message: String,
}

#[derive(Debug, Deserialize)]
pub struct SendInquiryMessageRequest {
    pub sender_id: Option<String>,
    pub sender_name: Option<String>,
    pub sender_role: String, // "TEACHER" or "STUDENT"
    pub content: String,
}

pub fn inquiry_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", get(list_inquiries).post(create_inquiry))
        .route("/{id}", get(get_inquiry_detail))
        .route("/{id}/messages", post(send_message))
        .route("/{id}/resolve", post(resolve_inquiry))
}

async fn resolve_effective_tenant_id(pool: &sqlx::PgPool, candidate: Uuid) -> Uuid {
    let dummy = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    if candidate == dummy {
        if let Ok(Some(tid)) = sqlx::query_scalar!("SELECT tenant_id FROM inquiry_threads LIMIT 1")
            .fetch_optional(pool)
            .await
        {
            return tid;
        }
        if let Ok(Some(tid)) = sqlx::query_scalar!("SELECT id FROM tenants ORDER BY created_at ASC LIMIT 1")
            .fetch_optional(pool)
            .await
        {
            return tid;
        }
    }
    candidate
}

async fn list_inquiries(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ListInquiriesQuery>,
) -> Result<Json<ApiResponse<Vec<InquiryThreadDto>>>, ApiError> {
    let tenant_id = resolve_effective_tenant_id(&ctx.pool, req_ctx.tenant_id).await;

    // Check if the caller is a Teacher in this tenant
    let actor_teacher = if let Some(ref actor) = req_ctx.actor {
        sqlx::query!(
            "SELECT id, full_name FROM teachers WHERE user_id = $1 AND tenant_id = $2",
            actor.id,
            tenant_id
        )
        .fetch_optional(&ctx.pool)
        .await
        .unwrap_or(None)
    } else {
        None
    };

    let effective_teacher_id = query.teacher_id.or(actor_teacher.as_ref().map(|t| t.id));
    let effective_teacher_name = query.teacher_name
        .or(actor_teacher.as_ref().map(|t| t.full_name.clone()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let rows = sqlx::query!(
        r#"
        SELECT 
            t.id, t.student_id, t.student_name, t.student_class,
            t.teacher_id, t.teacher_name, t.subject_name, t.inquiry_type,
            t.reference_title, t.reference_id, t.status,
            t.last_message_content, t.last_message_at, t.created_at,
            (SELECT COUNT(*)::bigint FROM inquiry_messages m WHERE m.thread_id = t.id) as "message_count!"
        FROM inquiry_threads t
        WHERE t.tenant_id = $1
          AND ($2::text IS NULL OR t.status = $2)
          AND ($3::text IS NULL OR t.inquiry_type = $3)
          AND ($4::uuid IS NULL OR t.student_id = $4)
          AND (
              ($5::uuid IS NULL AND $6::text IS NULL) OR
              t.teacher_id = $5 OR
              ($6::text IS NOT NULL AND t.teacher_name ILIKE '%' || $6 || '%')
          )
          AND (
              $7::text IS NULL OR $7 = '' OR 
              t.student_name ILIKE '%' || $7 || '%' OR 
              t.reference_title ILIKE '%' || $7 || '%' OR 
              t.student_class ILIKE '%' || $7 || '%' OR
              t.subject_name ILIKE '%' || $7 || '%' OR
              t.last_message_content ILIKE '%' || $7 || '%'
          )
        ORDER BY t.last_message_at DESC
        "#,
        tenant_id,
        query.status,
        query.inquiry_type,
        query.student_id,
        effective_teacher_id,
        effective_teacher_name,
        query.search.as_deref().map(|s| s.trim())
    )
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
        .map(|r| InquiryThreadDto {
            id: r.id,
            student_id: r.student_id,
            student_name: r.student_name,
            student_class: r.student_class,
            teacher_id: r.teacher_id,
            teacher_name: r.teacher_name,
            subject_name: r.subject_name,
            inquiry_type: r.inquiry_type,
            reference_title: r.reference_title,
            reference_id: r.reference_id,
            status: r.status,
            last_message_content: r.last_message_content,
            last_message_at: r.last_message_at,
            created_at: r.created_at,
            message_count: r.message_count,
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_inquiry_detail(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<InquiryDetailDto>>, ApiError> {
    let thread = sqlx::query!(
        r#"
        SELECT 
            t.id, t.student_id, t.student_name, t.student_class,
            t.teacher_id, t.teacher_name, t.subject_name, t.inquiry_type,
            t.reference_title, t.reference_id, t.status,
            t.last_message_content, t.last_message_at, t.created_at,
            (SELECT COUNT(*)::bigint FROM inquiry_messages m WHERE m.thread_id = t.id) as "message_count!"
        FROM inquiry_threads t
        WHERE t.id = $1
        "#,
        id
    )
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
                school_core::common::error_code::ErrorCode::ResourceNotFound,
                "Inquiry thread not found".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let messages = sqlx::query!(
        r#"
        SELECT id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at
        FROM inquiry_messages
        WHERE thread_id = $1
        ORDER BY created_at ASC
        "#,
        id
    )
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

    let dto = InquiryDetailDto {
        thread: InquiryThreadDto {
            id: thread.id,
            student_id: thread.student_id,
            student_name: thread.student_name,
            student_class: thread.student_class,
            teacher_id: thread.teacher_id,
            teacher_name: thread.teacher_name,
            subject_name: thread.subject_name,
            inquiry_type: thread.inquiry_type,
            reference_title: thread.reference_title,
            reference_id: thread.reference_id,
            status: thread.status,
            last_message_content: thread.last_message_content,
            last_message_at: thread.last_message_at,
            created_at: thread.created_at,
            message_count: thread.message_count,
        },
        messages: messages
            .into_iter()
            .map(|m| InquiryMessageDto {
                id: m.id,
                thread_id: m.thread_id,
                sender_id: m.sender_id,
                sender_name: m.sender_name,
                sender_role: m.sender_role,
                content: m.content,
                is_from_teacher: m.is_from_teacher,
                created_at: m.created_at,
            })
            .collect(),
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

async fn create_inquiry(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateInquiryRequest>,
) -> Result<Json<ApiResponse<InquiryThreadDto>>, ApiError> {
    if payload.reference_title.trim().is_empty() || payload.initial_message.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(school_core::common::error::DomainError::Validation(
                "Judul referensi dan pesan awal wajib diisi".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    // Resolve student
    let (student_id, student_name, student_class) = if let Some(sid) = payload.student_id {
        let std_info = sqlx::query!(
            r#"
            SELECT s.id, s.full_name, COALESCE(c.name, 'Kelas Siswa') as "class_name!"
            FROM students s
            LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
            LEFT JOIN classes c ON c.id = en.class_id
            WHERE s.id = $1 AND s.tenant_id = $2
            LIMIT 1
            "#,
            sid,
            req_ctx.tenant_id
        )
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        if let Some(s) = std_info {
            (s.id, s.full_name, s.class_name)
        } else {
            (sid, payload.student_name.unwrap_or_else(|| "Siswa".to_string()), payload.student_class.unwrap_or_else(|| "Siswa".to_string()))
        }
    } else {
        // Find from actor user_id
        let std_info = sqlx::query!(
            r#"
            SELECT s.id, s.full_name, COALESCE(c.name, 'Kelas Siswa') as "class_name!"
            FROM students s
            LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
            LEFT JOIN classes c ON c.id = en.class_id
            WHERE s.user_id = $1 AND s.tenant_id = $2
            LIMIT 1
            "#,
            actor_id,
            req_ctx.tenant_id
        )
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        if let Some(s) = std_info {
            (s.id, s.full_name, s.class_name)
        } else {
            // Fallback to first student in tenant if test/demo
            let first_std = sqlx::query!(
                r#"
                SELECT s.id, s.full_name, COALESCE(c.name, 'PAKET B7') as "class_name!"
                FROM students s
                LEFT JOIN enrollments en ON en.student_id = s.id
                LEFT JOIN classes c ON c.id = en.class_id
                WHERE s.tenant_id = $1
                LIMIT 1
                "#,
                req_ctx.tenant_id
            )
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            if let Some(s) = first_std {
                (s.id, s.full_name, s.class_name)
            } else {
                return Err(ApiError::new(
                    ApplicationError::NotFound(
                        school_core::common::error_code::ErrorCode::ResourceNotFound,
                        "Data siswa tidak ditemukan".to_string(),
                    ),
                    &req_ctx.request_id,
                ));
            }
        }
    };

    let tenant_id = resolve_effective_tenant_id(&ctx.pool, req_ctx.tenant_id).await;
    let mut resolved_tenant_id = tenant_id;
    let mut resolved_teacher_id = payload.teacher_id;
    let mut resolved_teacher_name = payload.teacher_name.unwrap_or_else(|| "Guru Pengampu".to_string());
    let mut resolved_subject_name = payload.subject_name.unwrap_or_else(|| "Umum".to_string());
    let mut resolved_class_name = student_class.clone();

    // Authority lookup: If reference_id is provided, resolve from authoritative learning_materials or assignments
    if let Some(ref ref_id_str) = payload.reference_id {
        if let Ok(ref_uuid) = Uuid::parse_str(ref_id_str) {
            if payload.inquiry_type.eq_ignore_ascii_case("MATERIAL") {
                if let Ok(Some(mat)) = sqlx::query!(
                    r#"
                    SELECT 
                        m.tenant_id, m.teacher_id, 
                        t.full_name as teacher_name, 
                        sub.name as subject_name,
                        c.name as class_name
                    FROM learning_materials m
                    LEFT JOIN teachers t ON t.id = m.teacher_id
                    LEFT JOIN subjects sub ON sub.id = m.subject_id
                    LEFT JOIN classes c ON c.id = m.class_id
                    WHERE m.id = $1
                    "#,
                    ref_uuid
                ).fetch_optional(&ctx.pool).await {
                    resolved_tenant_id = mat.tenant_id;
                    if let Some(tid) = mat.teacher_id { resolved_teacher_id = Some(tid); }
                    resolved_teacher_name = mat.teacher_name;
                    resolved_subject_name = mat.subject_name;
                    resolved_class_name = mat.class_name;
                }
            } else if payload.inquiry_type.eq_ignore_ascii_case("ASSIGNMENT") {
                if let Ok(Some(asg)) = sqlx::query!(
                    r#"
                    SELECT 
                        a.tenant_id, a.teacher_id, 
                        t.full_name as teacher_name, 
                        sub.name as subject_name,
                        c.name as class_name
                    FROM assignments a
                    LEFT JOIN teachers t ON t.id = a.teacher_id
                    LEFT JOIN subjects sub ON sub.id = a.subject_id
                    LEFT JOIN classes c ON c.id = a.class_id
                    WHERE a.id = $1
                    "#,
                    ref_uuid
                ).fetch_optional(&ctx.pool).await {
                    resolved_tenant_id = asg.tenant_id;
                    if let Some(tid) = asg.teacher_id { resolved_teacher_id = Some(tid); }
                    resolved_teacher_name = asg.teacher_name;
                    resolved_subject_name = asg.subject_name;
                    resolved_class_name = asg.class_name;
                }
            }
        }
    }

    // If teacher is still unresolved, resolve by subject name in the same tenant
    if resolved_teacher_id.is_none() {
        if let Ok(Some(tch)) = sqlx::query!(
            r#"
            SELECT id, full_name
            FROM teachers
            WHERE tenant_id = $1
              AND (
                  (subject IS NOT NULL AND subject ILIKE '%' || $2 || '%') OR
                  full_name ILIKE '%' || $3 || '%'
              )
            ORDER BY created_at ASC
            LIMIT 1
            "#,
            resolved_tenant_id,
            resolved_subject_name,
            resolved_teacher_name
        )
        .fetch_optional(&ctx.pool)
        .await
        {
            resolved_teacher_id = Some(tch.id);
            resolved_teacher_name = tch.full_name;
        }
    }

    let thread_id = Uuid::new_v4();
    let inquiry_type = payload.inquiry_type.to_uppercase();
    let initial_msg = payload.initial_message.trim().to_string();

    let thread = sqlx::query!(
        r#"
        INSERT INTO inquiry_threads (
            id, tenant_id, student_id, student_name, student_class,
            teacher_id, teacher_name, subject_name, inquiry_type,
            reference_title, reference_id, status, last_message_content,
            last_message_at, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'WAITING_REPLY', $12, NOW(), NOW(), NOW())
        RETURNING id, student_id, student_name, student_class, teacher_id, teacher_name,
                  subject_name, inquiry_type, reference_title, reference_id, status,
                  last_message_content, last_message_at, created_at
        "#,
        thread_id,
        resolved_tenant_id,
        student_id,
        student_name,
        resolved_class_name,
        resolved_teacher_id,
        resolved_teacher_name,
        resolved_subject_name,
        inquiry_type,
        payload.reference_title.trim(),
        payload.reference_id,
        initial_msg
    )
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

    // Insert first message
    let _ = sqlx::query!(
        r#"
        INSERT INTO inquiry_messages (id, tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, 'STUDENT', $5, false, NOW())
        "#,
        tenant_id,
        thread_id,
        student_id.to_string(),
        student_name,
        initial_msg
    )
    .execute(&ctx.pool)
    .await;

    let dto = InquiryThreadDto {
        id: thread.id,
        student_id: thread.student_id,
        student_name: thread.student_name,
        student_class: thread.student_class,
        teacher_id: thread.teacher_id,
        teacher_name: thread.teacher_name,
        subject_name: thread.subject_name,
        inquiry_type: thread.inquiry_type,
        reference_title: thread.reference_title,
        reference_id: thread.reference_id,
        status: thread.status,
        last_message_content: thread.last_message_content,
        last_message_at: thread.last_message_at,
        created_at: thread.created_at,
        message_count: 1,
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

async fn send_message(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SendInquiryMessageRequest>,
) -> Result<Json<ApiResponse<InquiryMessageDto>>, ApiError> {
    if payload.content.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(school_core::common::error::DomainError::Validation(
                "Pesan tidak boleh kosong".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let thread = sqlx::query!(
        "SELECT tenant_id FROM inquiry_threads WHERE id = $1",
        id
    )
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
                school_core::common::error_code::ErrorCode::ResourceNotFound,
                "Inquiry thread not found".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let effective_tenant_id = thread.tenant_id;
    let is_teacher = payload.sender_role.eq_ignore_ascii_case("TEACHER");
    let sender_id = payload.sender_id.unwrap_or_else(|| {
        req_ctx.actor.as_ref().map(|a| a.id.to_string()).unwrap_or_else(|| "user-1".to_string())
    });
    let sender_name = payload.sender_name.unwrap_or_else(|| {
        if is_teacher { "Guru Pengampu".to_string() } else { "Siswa".to_string() }
    });

    let message_id = Uuid::new_v4();
    let content = payload.content.trim().to_string();

    let msg = sqlx::query!(
        r#"
        INSERT INTO inquiry_messages (id, tenant_id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        RETURNING id, thread_id, sender_id, sender_name, sender_role, content, is_from_teacher, created_at
        "#,
        message_id,
        effective_tenant_id,
        id,
        sender_id,
        sender_name,
        payload.sender_role.to_uppercase(),
        content,
        is_teacher
    )
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

    // Update thread status & timestamp
    let new_status = if is_teacher { "ANSWERED" } else { "WAITING_REPLY" };
    let _ = sqlx::query!(
        r#"
        UPDATE inquiry_threads
        SET status = $1, last_message_content = $2, last_message_at = NOW(), updated_at = NOW()
        WHERE id = $3
        "#,
        new_status,
        content,
        id
    )
    .execute(&ctx.pool)
    .await;

    let dto = InquiryMessageDto {
        id: msg.id,
        thread_id: msg.thread_id,
        sender_id: msg.sender_id,
        sender_name: msg.sender_name,
        sender_role: msg.sender_role,
        content: msg.content,
        is_from_teacher: msg.is_from_teacher,
        created_at: msg.created_at,
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

async fn resolve_inquiry(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    sqlx::query!(
        r#"
        UPDATE inquiry_threads
        SET status = 'ANSWERED', updated_at = NOW()
        WHERE id = $1
        "#,
        id
    )
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

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}
