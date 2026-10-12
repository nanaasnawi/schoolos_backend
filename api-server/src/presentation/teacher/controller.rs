use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, InfrastructureError};

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReminderStatusDto {
    pub id: Uuid,
    pub sent: bool,
    pub sent_at: String,
    pub title: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TeacherReminderHistoryResponse {
    pub data: HashMap<String, ReminderStatusDto>,
    pub recent_list: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SendTeacherReminderPayload {
    pub student_ids: Option<Vec<Uuid>>,
    pub student_id: Option<Uuid>,
    pub title: Option<String>,
    pub reason: Option<String>,
    pub custom_message: Option<String>,
    pub action_label: Option<String>,
    pub action_url: Option<String>,
    pub send_fcm: Option<bool>,
}

pub fn teacher_workspace_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/remind", get(get_teacher_reminders).post(send_teacher_reminders))
}

pub async fn get_teacher_reminders(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<TeacherReminderHistoryResponse>>, ApiError> {
    let query = r#"
        SELECT n.id, n.user_id, n.title, n.body, n.created_at, n.is_read, n.reference_id,
               s.id as student_id, s.full_name as student_name
        FROM notifications n
        JOIN students s ON s.user_id = n.user_id
        WHERE n.tenant_id = $1
          AND n.notification_type IN ('SMART_REMINDER', 'MATERIAL_REMINDER', 'ASSIGNMENT_REMINDER', 'AT_RISK_REMINDER')
          AND n.created_at >= NOW() - INTERVAL '7 days'
        ORDER BY n.created_at DESC
    "#;

    let rows = sqlx::query(query)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let mut reminders_map: HashMap<String, ReminderStatusDto> = HashMap::new();
    let mut recent_list = Vec::new();

    for row in rows {
        let student_id: Uuid = row.get("student_id");
        let id: Uuid = row.get("id");
        let title: String = row.get("title");
        let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");

        let key = student_id.to_string();
        if !reminders_map.contains_key(&key) {
            reminders_map.insert(
                key,
                ReminderStatusDto {
                    id,
                    sent: true,
                    sent_at: created_at.to_rfc3339(),
                    title: title.clone(),
                },
            );
        }

        recent_list.push(serde_json::json!({
            "id": id,
            "user_id": row.get::<Uuid, _>("user_id"),
            "student_id": student_id,
            "student_name": row.get::<String, _>("student_name"),
            "title": title,
            "body": row.get::<String, _>("body"),
            "is_read": row.get::<bool, _>("is_read"),
            "created_at": created_at.to_rfc3339(),
        }));
    }

    Ok(Json(ApiResponse::success(
        TeacherReminderHistoryResponse {
            data: reminders_map,
            recent_list,
        },
        req_ctx.request_id,
    )))
}

pub async fn send_teacher_reminders(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SendTeacherReminderPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let mut target_student_ids = payload.student_ids.unwrap_or_default();
    if let Some(single_id) = payload.student_id {
        if !target_student_ids.contains(&single_id) {
            target_student_ids.push(single_id);
        }
    }

    if target_student_ids.is_empty() {
        return Ok(Json(ApiResponse::success(
            serde_json::json!({ "success": false, "message": "Tidak ada siswa yang dipilih." }),
            req_ctx.request_id,
        )));
    }

    let title = payload.title.unwrap_or_else(|| "Pengingat Pembelajaran".to_string());
    let body_text = payload.custom_message.or(payload.reason).unwrap_or_else(|| {
        "Guru Anda mengirimkan pengingat untuk segera menyelesaikan aktivitas belajar di SchoolOS.".to_string()
    });

    // Cari user_id siswa yang sesuai di tenant yang sama
    let students = sqlx::query(
        "SELECT id, user_id, full_name FROM students WHERE id = ANY($1) AND tenant_id = $2 AND deleted_at IS NULL"
    )
    .bind(&target_student_ids)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let mut sent_count = 0;
    for s in students {
        let user_id: Option<Uuid> = s.try_get("user_id").ok().flatten();
        if let Some(uid) = user_id {
            let notif_id = Uuid::new_v4();
            let _ = sqlx::query(
                r#"
                INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, is_read, created_at)
                VALUES ($1, $2, $3, $4, $5, 'SMART_REMINDER', false, NOW())
                "#
            )
            .bind(notif_id)
            .bind(req_ctx.tenant_id)
            .bind(uid)
            .bind(&title)
            .bind(&body_text)
            .execute(&ctx.pool)
            .await;

            sent_count += 1;
        }
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "success": true,
            "sent_count": sent_count,
            "message": format!("Pengingat berhasil dikirimkan ke {} siswa.", sent_count),
        }),
        req_ctx.request_id,
    )))
}
