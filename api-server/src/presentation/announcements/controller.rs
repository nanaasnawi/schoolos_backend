use axum::{
    Json, Router,
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, patch},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::LazyLock;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnnouncementTarget {
    TargetAll,
    TargetStudent,
    TargetTeacher,
    TargetGuardian,
}

impl AnnouncementTarget {
    pub fn parse(val: &str) -> Self {
        let v = val.trim().to_uppercase();
        if v == "TARGET_STUDENT"
            || v == "STUDENT"
            || (v.contains("SISWA") && !v.contains("SEMUA") && !v.contains("GURU"))
        {
            Self::TargetStudent
        } else if v == "TARGET_GUARDIAN"
            || v == "GUARDIAN"
            || v.contains("WALI")
            || v.contains("ORANG TUA")
        {
            Self::TargetGuardian
        } else if v == "TARGET_TEACHER"
            || v == "TEACHER"
            || (v.contains("GURU") && !v.contains("SEMUA") && !v.contains("SISWA"))
        {
            Self::TargetTeacher
        } else {
            Self::TargetAll
        }
    }

    pub fn to_filter_str(&self) -> &'static str {
        match self {
            Self::TargetAll => "ALL",
            Self::TargetStudent => "STUDENT",
            Self::TargetGuardian => "GUARDIAN",
            Self::TargetTeacher => "TEACHER",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::TargetAll => "Semua Siswa & Guru",
            Self::TargetStudent => "Siswa Mobile Android",
            Self::TargetGuardian => "Wali Murid / Orang Tua",
            Self::TargetTeacher => "Dewan Guru",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementResponse {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub category: String,
    pub target: String,
    pub author: String,
    pub is_pinned: bool,
    pub push_status: bool,
    pub date: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub content: String,
    pub category: Option<String>,
    pub target: Option<String>,
    pub author: Option<String>,
    pub is_pinned: Option<bool>,
    pub send_push: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CreateAnnouncementResponse {
    pub announcement: AnnouncementResponse,
    pub notifications_sent: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnouncementBroadcastEvent {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub title: String,
    pub content: String,
    pub category: String,
    pub target: String,
    pub author: String,
    pub date: String,
    pub is_pinned: bool,
}

static ANNOUNCEMENT_BROADCAST: LazyLock<broadcast::Sender<AnnouncementBroadcastEvent>> =
    LazyLock::new(|| {
        let (tx, _rx) = broadcast::channel(512);
        tx
    });

pub fn announcement_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/stream", get(stream_announcements))
        .route("/{id}", delete(delete_announcement))
        .route("/{id}/pin", patch(toggle_pin))
}

async fn stream_announcements(
    req_ctx: RequestContext,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let tenant_id = req_ctx.tenant_id;
    let rx = ANNOUNCEMENT_BROADCAST.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(move |item| match item {
        Ok(event) if event.tenant_id == tenant_id => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().event("announcement").data(json)))
        }
        _ => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AnnouncementResponse>>>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT id, title, content, category, target, author, is_pinned, push_status, created_at
        FROM announcements
        WHERE tenant_id = $1
        ORDER BY is_pinned DESC, created_at DESC
        LIMIT 100
        "#,
        req_ctx.tenant_id
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

    let items: Vec<AnnouncementResponse> = rows
        .into_iter()
        .map(|r| {
            let date_str = r.created_at.format("%d %b %Y · %H:%M WIB").to_string();
            AnnouncementResponse {
                id: r.id,
                title: r.title,
                content: r.content,
                category: r.category,
                target: r.target,
                author: r.author,
                is_pinned: r.is_pinned,
                push_status: r.push_status,
                date: date_str,
                created_at: r.created_at,
            }
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAnnouncementRequest>,
) -> Result<Json<ApiResponse<CreateAnnouncementResponse>>, ApiError> {
    if payload.title.trim().is_empty() || payload.content.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(school_core::common::error::DomainError::Validation(
                "Title and content cannot be empty".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let announcement_id = Uuid::new_v4();
    let category = payload.category.unwrap_or_else(|| "AKADEMIK".to_string());
    let raw_target = payload.target.unwrap_or_else(|| "TARGET_ALL".to_string());
    let target_enum = AnnouncementTarget::parse(&raw_target);
    let target_display = target_enum.display_label().to_string();
    let author = payload
        .author
        .unwrap_or_else(|| "Kepala Sekolah".to_string());
    let is_pinned = payload.is_pinned.unwrap_or(false);
    let send_push = payload.send_push.unwrap_or(true);

    let row = sqlx::query!(
        r#"
        INSERT INTO announcements (id, tenant_id, title, content, category, target, author, is_pinned, push_status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, tenant_id, title, content, category, target, author, is_pinned, push_status, created_at
        "#,
        announcement_id,
        req_ctx.tenant_id,
        payload.title.trim(),
        payload.content.trim(),
        category,
        target_display,
        author,
        is_pinned,
        send_push
    )
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let mut notifications_sent = 0i64;

    if send_push {
        let target_filter = target_enum.to_filter_str();

        let insert_result = sqlx::query!(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, is_read, created_at)
            SELECT 
                gen_random_uuid(),
                u.tenant_id,
                u.id,
                $2 as title,
                $3 as body,
                'ANNOUNCEMENT',
                'in_app',
                FALSE,
                NOW()
            FROM users u
            WHERE u.tenant_id = $1
              AND u.is_active = TRUE
              AND (
                $4 = 'ALL'
                OR ($4 = 'STUDENT' AND EXISTS (
                    SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id AND (LOWER(r.name) LIKE '%siswa%' OR LOWER(r.name) LIKE '%student%')
                ))
                OR ($4 = 'GUARDIAN' AND EXISTS (
                    SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id AND (LOWER(r.name) LIKE '%wali%' OR LOWER(r.name) LIKE '%parent%' OR LOWER(r.name) LIKE '%ortu%')
                ))
                OR ($4 = 'TEACHER' AND EXISTS (
                    SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id 
                    WHERE ur.user_id = u.id AND (LOWER(r.name) LIKE '%guru%' OR LOWER(r.name) LIKE '%teacher%')
                ))
              )
            "#,
            req_ctx.tenant_id,
            row.title,
            row.content,
            target_filter
        )
        .execute(&ctx.pool)
        .await;

        if let Ok(res) = insert_result {
            notifications_sent = res.rows_affected() as i64;
        }
    }

    let date_str = row.created_at.format("%d %b %Y · %H:%M WIB").to_string();

    // ── TRIGGER REAL-TIME SSE BROADCAST EVENT ────────────────────────────────
    let broadcast_event = AnnouncementBroadcastEvent {
        id: row.id,
        tenant_id: row.tenant_id,
        title: row.title.clone(),
        content: row.content.clone(),
        category: row.category.clone(),
        target: row.target.clone(),
        author: row.author.clone(),
        date: date_str.clone(),
        is_pinned: row.is_pinned,
    };
    let _ = ANNOUNCEMENT_BROADCAST.send(broadcast_event);

    let resp = CreateAnnouncementResponse {
        announcement: AnnouncementResponse {
            id: row.id,
            title: row.title,
            content: row.content,
            category: row.category,
            target: row.target,
            author: row.author,
            is_pinned: row.is_pinned,
            push_status: row.push_status,
            date: date_str,
            created_at: row.created_at,
        },
        notifications_sent,
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn toggle_pin(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    let res = sqlx::query!(
        r#"
        UPDATE announcements
        SET is_pinned = NOT is_pinned, updated_at = NOW()
        WHERE id = $1 AND tenant_id = $2
        RETURNING is_pinned
        "#,
        id,
        req_ctx.tenant_id
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
    })?;

    let is_pinned = res.map(|r| r.is_pinned).unwrap_or(false);
    Ok(Json(ApiResponse::success(is_pinned, req_ctx.request_id)))
}

async fn delete_announcement(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    sqlx::query!(
        r#"
        DELETE FROM announcements
        WHERE id = $1 AND tenant_id = $2
        "#,
        id,
        req_ctx.tenant_id
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
