use axum::{
    extract::{Query, State},
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
    response::ApiResponse,
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
    pub quiz_id: Option<Uuid>,
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
            "QUIZ_ALERT" | "UNATTEMPTED_QUIZ" | "QUIZ_LOW_SCORE" => {
                let t = format!("[UJIAN & KUIS CBT] {}", title_req);
                let b = format!(
                    "Halo {}, Bapak/Ibu Guru {} mengingatkan Anda mengenai evaluasi kuis/ujian CBT \"{}\". Segera akses dan selesaikan ujian Anda.",
                    student_name, teacher_name, title_req
                );
                (t, b, "quiz", payload.quiz_id, FcmCategory::Quiz)
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

#[derive(Debug, Serialize, Deserialize)]
pub struct AtRiskStudentResponse {
    pub student_id: Uuid,
    pub student_name: String,
    pub nisn: Option<String>,
    pub class_id: Option<Uuid>,
    pub class_name: String,
    pub category: String, // "UNREAD_MATERIAL" | "OVERDUE_ASSIGNMENT" | "LOW_SCORE" | "QUIZ_ALERT"
    pub category_label: String,
    pub risk_level: String, // "HIGH" | "MEDIUM" | "LOW"
    pub title: String,
    pub description: String,
    pub action_label: String,
    pub action_type: String,
    pub target_url: Option<String>,
    pub material_id: Option<Uuid>,
    pub assignment_id: Option<Uuid>,
    pub quiz_id: Option<Uuid>,
    pub is_reminded: bool,
    pub reminded_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AtRiskQuery {
    pub class_id: Option<Uuid>,
}

/// GET /api/v1/teacher/at-risk-students or /api/v1/teachers/at-risk-students
/// Mengembalikan daftar siswa perlu perhatian terpadu (Materi, Tugas, Kuis & CBT, Nilai KKTP)
/// Terintegrasi untuk Dashboard Kepala Sekolah, Workstation Guru, dan Mobile Android
pub async fn get_at_risk_students(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<AtRiskQuery>,
) -> Result<Json<ApiResponse<Vec<AtRiskStudentResponse>>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_admin = req_ctx.actor.as_ref().map(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala") || n.contains("headmaster") || n.contains("staff")
        })
    }).unwrap_or(false);

    let teacher_id: Option<Uuid> = if !is_admin {
        sqlx::query_scalar(
            "SELECT id FROM teachers WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 LIMIT 1",
        )
        .bind(actor_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // 1. Fetch active reminders in last 7 days
    let reminder_rows = sqlx::query(
        r#"
        SELECT s.id as student_id, MAX(n.created_at) as reminded_at
        FROM notifications n
        JOIN students s ON s.user_id = n.user_id AND s.tenant_id = n.tenant_id
        WHERE n.notification_type IN ('SMART_REMINDER', 'MATERIAL_REMINDER', 'ASSIGNMENT_REMINDER', 'QUIZ_REMINDER', 'AT_RISK_REMINDER')
          AND n.tenant_id = $1
          AND n.created_at >= NOW() - INTERVAL '7 days'
        GROUP BY s.id
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let mut reminders_map: HashMap<Uuid, DateTime<Utc>> = HashMap::new();
    for r in reminder_rows {
        if let (Ok(sid), Ok(rat)) = (r.try_get::<Uuid, _>("student_id"), r.try_get::<DateTime<Utc>, _>("reminded_at")) {
            reminders_map.insert(sid, rat);
        }
    }

    let mut results = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();

    // 2. UNREAD_MATERIAL
    let mat_rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (s.id, m.id)
            s.id as student_id,
            s.full_name as student_name,
            COALESCE(s.nisn, u.username, '-') as nisn,
            c.id as class_id,
            c.name as class_name,
            m.id as material_id,
            m.title as material_title,
            COALESCE(rp.current_page, 1) as current_page,
            COALESCE(rp.last_read_at, m.created_at) as updated_at
        FROM learning_materials m
        JOIN classes c ON c.id = m.class_id
        JOIN enrollments en ON en.class_id = c.id AND (en.status ILIKE 'active' OR en.status IS NULL)
        JOIN students s ON s.id = en.student_id AND s.deleted_at IS NULL AND s.is_active = true
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = m.id
        LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = m.id
        WHERE m.tenant_id = $1
          AND m.deleted_at IS NULL
          AND m.is_active = true
          AND (m.visibility = 'published' OR m.visibility = 'active')
          AND (m.release_at IS NULL OR m.release_at <= NOW())
          AND ($2::uuid IS NULL OR m.class_id = $2)
          AND (
              $3::bool = true 
              OR m.created_by = $4 
              OR ($5::uuid IS NOT NULL AND m.teacher_id = $5)
              OR m.teacher_id = $4
          )
          AND smc.id IS NULL
          AND (rp.is_completed IS NULL OR rp.is_completed = false)
          AND COALESCE(rp.current_page, 1) < 3
        ORDER BY s.id, m.id, m.created_at DESC
        LIMIT 60
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(query.class_id)
    .bind(is_admin)
    .bind(actor_id)
    .bind(teacher_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in mat_rows {
        let sid: Uuid = r.get("student_id");
        let mid: Uuid = r.get("material_id");
        let key = format!("{}_UNREAD_{}", sid, mid);
        if seen_keys.insert(key) {
            let mtitle: String = r.get("material_title");
            let cpage: i32 = r.get("current_page");
            let uat: DateTime<Utc> = r.get("updated_at");
            let rem_at = reminders_map.get(&sid);
            results.push(AtRiskStudentResponse {
                student_id: sid,
                student_name: r.get("student_name"),
                nisn: r.try_get("nisn").ok(),
                class_id: r.try_get("class_id").ok(),
                class_name: r.get("class_name"),
                category: "UNREAD_MATERIAL".to_string(),
                category_label: "Materi Tertinggal".to_string(),
                risk_level: "MEDIUM".to_string(),
                title: format!("Belum Membaca Materi {}", mtitle),
                description: format!("Siswa baru membaca sampai halaman {}. Diperlukan pengingat literasi.", cpage),
                action_label: "Kirim Pengingat".to_string(),
                action_type: "REMIND_STUDENT".to_string(),
                target_url: Some(format!("/dashboard/learning/materials?id={}", mid)),
                material_id: Some(mid),
                assignment_id: None,
                quiz_id: None,
                is_reminded: rem_at.is_some(),
                reminded_at: rem_at.map(|d| d.to_rfc3339()),
                updated_at: uat.to_rfc3339(),
            });
        }
    }

    // 3. OVERDUE_ASSIGNMENT (Siswa belum mengumpulkan tugas aktif)
    let assign_rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (s.id, a.id)
            s.id as student_id,
            s.full_name as student_name,
            COALESCE(s.nisn, u.username, '-') as nisn,
            c.id as class_id,
            c.name as class_name,
            a.id as assignment_id,
            a.title as assignment_title,
            a.due_at,
            a.created_at as updated_at
        FROM assignments a
        JOIN classes c ON c.id = a.class_id
        JOIN enrollments en ON en.class_id = c.id AND (en.status ILIKE 'active' OR en.status IS NULL)
        JOIN students s ON s.id = en.student_id AND s.deleted_at IS NULL AND s.is_active = true
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN assignment_submissions sub ON sub.assignment_id = a.id AND (sub.student_id = s.id OR (s.user_id IS NOT NULL AND sub.student_id = s.user_id))
        WHERE a.tenant_id = $1
          AND a.deleted_at IS NULL
          AND a.is_active = true
          AND (a.status = 'published' OR a.status = 'active')
          AND (a.release_at IS NULL OR a.release_at <= NOW())
          AND ($2::uuid IS NULL OR a.class_id = $2)
          AND (
              $3::bool = true 
              OR a.created_by = $4 
              OR ($5::uuid IS NOT NULL AND a.teacher_id = $5)
              OR a.teacher_id = $4
          )
          AND (sub.id IS NULL OR sub.status = 'unsubmitted' OR sub.submitted_at IS NULL)
        ORDER BY s.id, a.id, a.created_at DESC
        LIMIT 60
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(query.class_id)
    .bind(is_admin)
    .bind(actor_id)
    .bind(teacher_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in assign_rows {
        let sid: Uuid = r.get("student_id");
        let aid: Uuid = r.get("assignment_id");
        let key = format!("{}_OVERDUE_{}", sid, aid);
        if seen_keys.insert(key) {
            let atitle: String = r.get("assignment_title");
            let due_at: Option<DateTime<Utc>> = r.try_get("due_at").ok();
            let is_past_due = due_at.map(|d| d < Utc::now()).unwrap_or(false);
            let uat: DateTime<Utc> = r.get("updated_at");
            let rem_at = reminders_map.get(&sid);
            results.push(AtRiskStudentResponse {
                student_id: sid,
                student_name: r.get("student_name"),
                nisn: r.try_get("nisn").ok(),
                class_id: r.try_get("class_id").ok(),
                class_name: r.get("class_name"),
                category: "OVERDUE_ASSIGNMENT".to_string(),
                category_label: "Tugas Tertunda".to_string(),
                risk_level: if is_past_due { "HIGH".to_string() } else { "MEDIUM".to_string() },
                title: format!("Belum Mengumpulkan {}", atitle),
                description: if is_past_due {
                    "Tugas telah melewati tenggat waktu dan belum dikumpulkan.".to_string()
                } else {
                    "Siswa belum mengunggah lembar jawaban tugas terstruktur ini.".to_string()
                },
                action_label: "Ingatkan Tugas".to_string(),
                action_type: "REMIND_STUDENT".to_string(),
                target_url: Some(format!("/dashboard/learning/assignments?id={}", aid)),
                material_id: None,
                assignment_id: Some(aid),
                quiz_id: None,
                is_reminded: rem_at.is_some(),
                reminded_at: rem_at.map(|d| d.to_rfc3339()),
                updated_at: uat.to_rfc3339(),
            });
        }
    }

    // 4. LOW_SCORE (Nilai tugas di bawah standar KKM < 70)
    let score_rows = sqlx::query(
        r#"
        SELECT 
            s.id as student_id,
            s.full_name as student_name,
            COALESCE(s.nisn, u.username, '-') as nisn,
            c.id as class_id,
            c.name as class_name,
            a.id as assignment_id,
            a.title as assignment_title,
            sub.score,
            COALESCE(sub.submitted_at, sub.updated_at, a.created_at) as updated_at
        FROM assignment_submissions sub
        JOIN assignments a ON a.id = sub.assignment_id
        JOIN students s ON s.id = sub.student_id OR (s.user_id IS NOT NULL AND s.user_id = sub.student_id)
        LEFT JOIN users u ON u.id = s.user_id
        JOIN classes c ON c.id = a.class_id
        WHERE sub.tenant_id = $1
          AND a.deleted_at IS NULL
          AND ($2::uuid IS NULL OR a.class_id = $2)
          AND (
              $3::bool = true 
              OR a.created_by = $4 
              OR ($5::uuid IS NOT NULL AND a.teacher_id = $5)
              OR a.teacher_id = $4
          )
          AND sub.score IS NOT NULL
          AND sub.score < 70
          AND sub.status != 'unsubmitted'
        ORDER BY sub.submitted_at DESC NULLS LAST
        LIMIT 60
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(query.class_id)
    .bind(is_admin)
    .bind(actor_id)
    .bind(teacher_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in score_rows {
        let sid: Uuid = r.get("student_id");
        let aid: Uuid = r.get("assignment_id");
        let key = format!("{}_LOW_SCORE_{}", sid, aid);
        if seen_keys.insert(key) {
            let atitle: String = r.get("assignment_title");
            let score: i32 = r.get("score");
            let uat: DateTime<Utc> = r.get("updated_at");
            let rem_at = reminders_map.get(&sid);
            results.push(AtRiskStudentResponse {
                student_id: sid,
                student_name: r.get("student_name"),
                nisn: r.try_get("nisn").ok(),
                class_id: r.try_get("class_id").ok(),
                class_name: r.get("class_name"),
                category: "LOW_SCORE".to_string(),
                category_label: "Nilai Rendah".to_string(),
                risk_level: if score < 60 { "HIGH".to_string() } else { "MEDIUM".to_string() },
                title: format!("Nilai {} di bawah KKTP ({}/100)", atitle, score),
                description: format!("Siswa memperoleh skor {}. Perlu diberikan bimbingan atau tugas remedial.", score),
                action_label: "Beri Remedial".to_string(),
                action_type: "ASSIGN_REMEDIAL".to_string(),
                target_url: Some(format!("/dashboard/learning/assignments?id={}", aid)),
                material_id: None,
                assignment_id: Some(aid),
                quiz_id: None,
                is_reminded: rem_at.is_some(),
                reminded_at: rem_at.map(|d| d.to_rfc3339()),
                updated_at: uat.to_rfc3339(),
            });
        }
    }

    // 5. QUIZ_ALERT (Kuis & Ujian CBT: Belum Mengerjakan atau Nilai di bawah KKM)
    let quiz_rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (s.id, q.id)
            s.id as student_id,
            s.full_name as student_name,
            COALESCE(s.nisn, u.username, '-') as nisn,
            c.id as class_id,
            c.name as class_name,
            q.id as quiz_id,
            q.title as quiz_title,
            q.passing_score,
            q.max_score,
            qa.score as attempt_score,
            CASE WHEN qa.id IS NULL THEN 'UNATTEMPTED' ELSE 'LOW_SCORE' END as alert_sub_type,
            COALESCE(qa.completed_at, qa.started_at, q.created_at) as updated_at
        FROM quizzes q
        JOIN classes c ON c.id = q.class_id
        JOIN enrollments en ON en.class_id = c.id AND (en.status ILIKE 'active' OR en.status IS NULL)
        JOIN students s ON s.id = en.student_id AND s.deleted_at IS NULL AND s.is_active = true
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN quiz_attempts qa ON qa.quiz_id = q.id AND (qa.student_id = s.id OR (s.user_id IS NOT NULL AND qa.student_id = s.user_id))
        WHERE q.tenant_id = $1
          AND q.deleted_at IS NULL
          AND q.is_active = true
          AND (q.status = 'published' OR q.status = 'active')
          AND (COALESCE(q.release_at, q.start_at) IS NULL OR COALESCE(q.release_at, q.start_at) <= NOW())
          AND ($2::uuid IS NULL OR q.class_id = $2)
          AND (
              $3::bool = true 
              OR q.created_by = $4 
              OR ($5::uuid IS NOT NULL AND q.teacher_id = $5)
              OR q.teacher_id = $4
          )
          AND (
              qa.id IS NULL 
              OR (qa.score IS NOT NULL AND qa.score < COALESCE(NULLIF(q.passing_score, 0), 70))
          )
        ORDER BY s.id, q.id, q.created_at DESC
        LIMIT 60
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(query.class_id)
    .bind(is_admin)
    .bind(actor_id)
    .bind(teacher_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in quiz_rows {
        let sid: Uuid = r.get("student_id");
        let qid: Uuid = r.get("quiz_id");
        let key = format!("{}_QUIZ_{}", sid, qid);
        if seen_keys.insert(key) {
            let qtitle: String = r.get("quiz_title");
            let sub_type: String = r.get("alert_sub_type");
            let attempt_score: Option<i32> = r.try_get("attempt_score").ok().flatten();
            let pass_score: i32 = r.try_get("passing_score").unwrap_or(70);
            let uat: DateTime<Utc> = r.get("updated_at");
            let rem_at = reminders_map.get(&sid);

            let (title, desc, risk_lvl) = if sub_type == "UNATTEMPTED" {
                (
                    format!("Belum Mengikuti Kuis/CBT: {}", qtitle),
                    "Siswa belum memulai ujian atau kuis CBT ini.".to_string(),
                    "HIGH".to_string(),
                )
            } else {
                let sc = attempt_score.unwrap_or(0);
                (
                    format!("Nilai Kuis/CBT {} di bawah KKM ({})", qtitle, sc),
                    format!("Skor kuis CBT ({}) di bawah batas kelulusan (KKM {}). Perlu remedial.", sc, pass_score),
                    if sc < 60 { "HIGH".to_string() } else { "MEDIUM".to_string() },
                )
            };

            results.push(AtRiskStudentResponse {
                student_id: sid,
                student_name: r.get("student_name"),
                nisn: r.try_get("nisn").ok(),
                class_id: r.try_get("class_id").ok(),
                class_name: r.get("class_name"),
                category: "QUIZ_ALERT".to_string(),
                category_label: "Kuis & CBT".to_string(),
                risk_level: risk_lvl,
                title,
                description: desc,
                action_label: "Ingatkan Kuis".to_string(),
                action_type: "REMIND_STUDENT".to_string(),
                target_url: Some(format!("/dashboard/learning/quizzes?id={}", qid)),
                material_id: None,
                assignment_id: None,
                quiz_id: Some(qid),
                is_reminded: rem_at.is_some(),
                reminded_at: rem_at.map(|d| d.to_rfc3339()),
                updated_at: uat.to_rfc3339(),
            });
        }
    }

    Ok(Json(ApiResponse::success(results, req_ctx.request_id)))
}
