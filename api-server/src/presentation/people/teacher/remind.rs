use axum::{
    extract::State,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    infrastructure::fcm::{self, FcmCategory, FcmTarget},
};
use school_core::common::error::ApplicationError;

#[derive(Debug, Deserialize)]
pub struct SendReminderRequest {
    pub student_id: Option<Uuid>,
    pub student_ids: Option<Vec<Uuid>>,
    pub title: Option<String>,
    pub reason: Option<String>,
    pub category: Option<String>,
    pub material_id: Option<Uuid>,
    pub assignment_id: Option<Uuid>,
    pub teacher_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReminderRecordDto {
    pub notification_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub user_id: Uuid,
    pub sent_at: String,
}

#[derive(Debug, Serialize)]
pub struct SendReminderResponse {
    pub success: bool,
    pub message: String,
    pub sent_count: usize,
    pub sent_at: String,
    pub reminders: Vec<ReminderRecordDto>,
}

#[derive(Debug, Serialize)]
pub struct ReminderItemDto {
    pub id: Uuid,
    pub sent: bool,
    pub sent_at: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct GetRemindersResponse {
    pub success: bool,
    pub data: HashMap<String, ReminderItemDto>,
}

/// GET /api/v1/teacher/remind or /api/v1/teachers/remind
/// Memuat riwayat pengingat guru dalam 7 hari terakhir yang dikelompokkan berdasarkan student_id
pub async fn get_reminders(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<GetRemindersResponse>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT n.id, n.user_id, n.title, n.created_at, s.id as student_id
        FROM notifications n
        JOIN students s ON s.user_id = n.user_id AND s.tenant_id = n.tenant_id
        WHERE n.notification_type IN ('SMART_REMINDER', 'MATERIAL_REMINDER', 'ASSIGNMENT_REMINDER', 'AT_RISK_REMINDER')
          AND ($1::uuid = '00000000-0000-0000-0000-000000000000'::uuid OR n.tenant_id = $1)
          AND n.created_at >= NOW() - INTERVAL '7 days'
        ORDER BY n.created_at DESC
        "#,
    )
    .bind(req_ctx.tenant_id)
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

    let mut data = HashMap::new();
    for row in rows {
        let student_id: Option<Uuid> = row.try_get("student_id").ok();
        let notif_id: Uuid = row.try_get("id").unwrap_or_default();
        let title: String = row.try_get("title").unwrap_or_else(|_| "Pengingat Guru".to_string());
        let created_at: Option<DateTime<Utc>> = row.try_get("created_at").ok();

        if let Some(sid) = student_id {
            let key = sid.to_string();
            if !data.contains_key(&key) {
                data.insert(
                    key,
                    ReminderItemDto {
                        id: notif_id,
                        sent: true,
                        sent_at: created_at.map(|d| d.to_rfc3339()).unwrap_or_else(|| Utc::now().to_rfc3339()),
                        title,
                    },
                );
            }
        }
    }

    Ok(Json(GetRemindersResponse {
        success: true,
        data,
    }))
}

/// POST /api/v1/teacher/remind or /api/v1/teachers/remind
/// Mengirim notifikasi pengingat intervensi belajar secara realtime (in-app dan FCM push)
/// ke satu siswa atau serentak ke rombongan siswa tertinggal (At-Risk Monitoring)
pub async fn remind_students(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SendReminderRequest>,
) -> Result<Json<SendReminderResponse>, ApiError> {
    let mut target_ids = Vec::new();
    if let Some(list) = payload.student_ids {
        target_ids.extend(list);
    }
    if let Some(single) = payload.student_id {
        if !target_ids.contains(&single) {
            target_ids.push(single);
        }
    }

    if target_ids.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(school_core::common::error::DomainError::Validation(
                "Target siswa (student_ids) tidak boleh kosong".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let rows = sqlx::query(
        r#"
        SELECT s.id, s.user_id, s.full_name, s.tenant_id
        FROM students s
        WHERE s.id = ANY($1) 
          AND ($2::uuid = '00000000-0000-0000-0000-000000000000'::uuid OR s.tenant_id = $2) 
          AND s.deleted_at IS NULL
        "#,
    )
    .bind(&target_ids)
    .bind(req_ctx.tenant_id)
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

    if rows.is_empty() {
        return Err(ApiError::new(
            ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::StudentNotFound,
                "Data siswa tidak ditemukan di sistem".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let teacher_name = payload
        .teacher_name
        .clone()
        .unwrap_or_else(|| "Guru Pengampu".to_string());
    let title_req = payload
        .title
        .clone()
        .unwrap_or_else(|| "Pengingat Pembelajaran".to_string());
    let category = payload
        .category
        .clone()
        .unwrap_or_else(|| "UNREAD_MATERIAL".to_string());

    let mut inserted_reminders = Vec::new();
    let now = Utc::now();

    for row in rows {
        let student_id: Uuid = row.try_get("id").unwrap_or_default();
        let student_user_id: Option<Uuid> = row.try_get("user_id").ok();
        let student_name: String = row.try_get("full_name").unwrap_or_else(|_| "Peserta Didik".to_string());
        let tenant_id: Uuid = row.try_get("tenant_id").unwrap_or(req_ctx.tenant_id);

        let Some(user_id) = student_user_id else {
            continue;
        };

        let (notif_title, notif_body, ref_type, ref_id, fcm_cat) = match category.as_str() {
            "UNREAD_MATERIAL" => {
                let t = format!("[LITERASI MODUL] Segera Baca: {}", title_req);
                let b = format!(
                    "Halo {}, Bapak/Ibu Guru {} mengingatkan untuk segera membaca dan mempelajari modul \"{}\".",
                    student_name, teacher_name, title_req
                );
                (t, b, "material", payload.material_id, FcmCategory::Material)
            }
            "OVERDUE_ASSIGNMENT" => {
                let t = format!("[TUGAS TERTUNDA] Segera Kumpulkan: {}", title_req);
                let b = format!(
                    "Halo {}, tugas \"{}\" telah melewati tenggat waktu. Segera kumpulkan jawaban Anda ke Bapak/Ibu Guru {}.",
                    student_name, title_req, teacher_name
                );
                (t, b, "assignment", payload.assignment_id, FcmCategory::Assignment)
            }
            "LOW_SCORE" => {
                let t = format!("[EVALUASI BELAJAR] Remedial: {}", title_req);
                let b = format!(
                    "Halo {}, nilai evaluasi \"{}\" masih di bawah standar KKM. Silakan hubungi Bapak/Ibu Guru {} untuk bimbingan remedial.",
                    student_name, title_req, teacher_name
                );
                (t, b, "assignment", payload.assignment_id, FcmCategory::Assignment)
            }
            _ => {
                let t = format!("[PENGINGAT GURU] {}", title_req);
                let b = payload.reason.clone().unwrap_or_else(|| {
                    format!(
                        "Bapak/Ibu Guru {} mengingatkan Anda untuk segera menyelesaikan kendala belajar: \"{}\". Tetap semangat belajar!",
                        teacher_name, title_req
                    )
                });
                (t, b, "reminder", None, FcmCategory::Reminder)
            }
        };

        let notif_id = Uuid::new_v4();

        let _ = sqlx::query(
            r#"
            INSERT INTO notifications (
                id, tenant_id, user_id, title, body, notification_type,
                channel, reference_type, reference_id, is_read, is_urgent,
                priority, scheduled_at, created_at
            ) VALUES (
                $1, $2, $3, $4, $5, 'SMART_REMINDER',
                'in_app', $6, $7, false, true,
                'HIGH', NOW(), NOW()
            )
            "#,
        )
        .bind(notif_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(&notif_title)
        .bind(&notif_body)
        .bind(ref_type)
        .bind(ref_id)
        .execute(&ctx.pool)
        .await;

        // Push targeted FCM notification exclusively to this student
        fcm::trigger_fcm_push_targeted(
            notif_title,
            notif_body,
            fcm_cat,
            ref_id.unwrap_or(notif_id),
            FcmTarget::User(user_id),
        );

        inserted_reminders.push(ReminderRecordDto {
            notification_id: notif_id,
            student_id,
            student_name,
            user_id,
            sent_at: now.to_rfc3339(),
        });
    }

    let count = inserted_reminders.len();
    Ok(Json(SendReminderResponse {
        success: true,
        message: format!("Pengingat berhasil dikirim ke {} siswa secara realtime.", count),
        sent_count: count,
        sent_at: now.to_rfc3339(),
        reminders: inserted_reminders,
    }))
}
