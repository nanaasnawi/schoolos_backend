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
              $5::text IS NULL OR $5 = '' OR 
              t.student_name ILIKE '%' || $5 || '%' OR 
              t.reference_title ILIKE '%' || $5 || '%' OR 
              t.student_class ILIKE '%' || $5 || '%' OR
              t.subject_name ILIKE '%' || $5 || '%' OR
              t.last_message_content ILIKE '%' || $5 || '%'
          )
        ORDER BY t.last_message_at DESC
        "#,
        tenant_id,
        query.status,
        query.inquiry_type,
        query.student_id,
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
    let thread_id = Uuid::new_v4();
    let teacher_name = payload.teacher_name.unwrap_or_else(|| "Guru Pengampu".to_string());
    let subject_name = payload.subject_name.unwrap_or_else(|| "Umum".to_string());
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
        tenant_id,
        student_id,
        student_name,
        student_class,
        payload.teacher_id,
        teacher_name,
        subject_name,
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
