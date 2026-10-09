use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDate, NaiveTime, Utc};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    attendance_response::AttendanceResponse, record_attendance_request::RecordAttendanceRequest,
    session_response::SessionResponse, start_session_request::StartSessionRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::session::{
    end_session::EndSessionCommand, get_attendance::GetAttendanceQuery,
    record_attendance::RecordAttendanceCommand,
    record_attendance_bulk::{AttendanceItemDto, RecordAttendanceBulkCommand},
};

pub fn session_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(start).get(list))
        .route("/{id}", get(get_by_id))
        .route("/{id}/end", post(end))
        .route("/{id}/cancel", post(cancel_session))
        .route("/{id}/substitute", post(substitute_teacher))
        .route(
            "/{id}/attendance",
            post(record_attendance).get(get_attendance),
        )
        .route("/{id}/attendance/bulk", post(record_attendance_bulk))
}

#[derive(Debug, Deserialize)]
pub struct CancelSessionPayload {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct SubstituteTeacherPayload {
    pub substitute_teacher_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SessionFilterQuery {
    pub class_id: Option<String>,
    pub teacher_id: Option<Uuid>,
    pub schedule_id: Option<Uuid>,
    pub date: Option<NaiveDate>,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub status: Option<String>,
}

fn parse_naive_time(s: &str) -> Option<NaiveTime> {
    let trimmed = s.trim();
    NaiveTime::parse_from_str(trimmed, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(trimmed, "%H:%M"))
        .ok()
}

async fn start(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<StartSessionRequest>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;

    // Guru, pengajar, dan admin berhak memulai sesi
    require_permission(&req_ctx.actor, Permission::LearningSessionCreate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to start learning session".to_string(),
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

    let now_utc = Utc::now();
    let today = now_utc.date_naive();
    let session_date = payload.session_date.unwrap_or(today);

    // Kasus 1: Sesi Terjadwal (Scheduled Session) dari Template Jadwal
    if let Some(sched_id) = payload.schedule_id {
        let schedule_row = sqlx::query(
            r#"
            SELECT cs.id, cs.tenant_id, cs.class_id, cs.subject_id, cs.teacher_id,
                   cs.start_time, cs.end_time, COALESCE(cs.room, 'Ruang Kelas') as room,
                   c.name as class_name, s.name as subject_name, t.full_name as teacher_name
            FROM class_schedules cs
            JOIN classes c ON c.id = cs.class_id
            JOIN subjects s ON s.id = cs.subject_id
            JOIN teachers t ON t.id = cs.teacher_id
            WHERE cs.id = $1 AND cs.tenant_id = $2 AND cs.deleted_at IS NULL
            "#,
        )
        .bind(sched_id)
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
        })?
        .ok_or_else(|| {
            ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::ResourceNotFound,
                    format!("Jadwal {} tidak ditemukan", sched_id),
                ),
                &req_ctx.request_id,
            )
        })?;

        let sched_class_id: Uuid = schedule_row.try_get("class_id").unwrap_or_default();
        let sched_subject_id: Uuid = schedule_row.try_get("subject_id").unwrap_or_default();
        let sched_teacher_id: Uuid = schedule_row.try_get("teacher_id").unwrap_or_default();
        let sched_start_time: String = schedule_row.try_get("start_time").unwrap_or_default();
        let sched_end_time: String = schedule_row.try_get("end_time").unwrap_or_default();
        let sched_class_name: String = schedule_row.try_get("class_name").unwrap_or_else(|_| "Kelas".to_string());
        let sched_subject_name: String = schedule_row.try_get("subject_name").unwrap_or_else(|_| "Pelajaran".to_string());

        let class_id = payload.class_id.unwrap_or(sched_class_id);
        let subject_id = payload.subject_id.or(Some(sched_subject_id));
        let teacher_id = payload.teacher_id.unwrap_or(sched_teacher_id);
        let substitute_teacher_id = payload.substitute_teacher_id;
        let session_number = payload.session_number.unwrap_or(1);

        let start_time_parsed = payload
            .start_time
            .as_deref()
            .and_then(parse_naive_time)
            .or_else(|| parse_naive_time(&sched_start_time));

        let end_time_parsed = payload
            .end_time
            .as_deref()
            .and_then(parse_naive_time)
            .or_else(|| parse_naive_time(&sched_end_time));

        let scheduled_at = start_time_parsed.and_then(|st| {
            session_date.and_time(st).and_local_timezone(Utc).single()
        });

        let new_session_id = Uuid::new_v4();

        // Idempotent atomic insert
        let inserted = sqlx::query(
            r#"
            INSERT INTO learning_sessions (
                id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
                substitute_teacher_id, session_date, session_number, start_time, end_time,
                scheduled_at, started_at, status, notes, created_at, updated_at
            )
            VALUES ($1, $2, 'scheduled', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), 'active', $14, NOW(), NOW())
            ON CONFLICT (tenant_id, schedule_id, session_date)
            WHERE schedule_id IS NOT NULL AND deleted_at IS NULL
            DO NOTHING
            RETURNING id
            "#
        )
        .bind(new_session_id)
        .bind(req_ctx.tenant_id)
        .bind(sched_id)
        .bind(payload.lesson_id)
        .bind(class_id)
        .bind(subject_id)
        .bind(teacher_id)
        .bind(substitute_teacher_id)
        .bind(session_date)
        .bind(session_number)
        .bind(start_time_parsed)
        .bind(end_time_parsed)
        .bind(scheduled_at)
        .bind(&payload.notes)
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

        let is_new_session = inserted.is_some();
        let final_session_id = match inserted {
            Some(ref row) => row.get::<Uuid, _>("id"),
            None => {
                // Konflik unique index -> ambil baris sesi yang sudah ada
                let existing_id: Uuid = sqlx::query_scalar(
                    r#"
                    SELECT id FROM learning_sessions
                    WHERE tenant_id = $1 AND schedule_id = $2 AND session_date = $3 AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(sched_id)
                .bind(session_date)
                .fetch_one(&ctx.pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        school_core::common::error::ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;
                existing_id
            }
        };

        // Kirim FCM jika baru dibuat
        if is_new_session {
            let tid = req_ctx.tenant_id;
            let cname = sched_class_name;
            let sname = sched_subject_name;
            let t = format!("🎓 Sesi Dimulai: {} - {}", cname, sname);
            let b = "Jadwal pelajaran dimulai. Presensi dibuka — segera bergabung!".to_string();
            let title_c = t.clone();
            let body_c = b.clone();
            let _ = sqlx::query(
                r#"
                INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                SELECT gen_random_uuid(), s.tenant_id, s.user_id, $1, $2,
                       'SESSION_STARTED', 'in_app', 'session', $4, FALSE, NOW()
                FROM students s
                JOIN enrollments en ON en.student_id = s.id
                WHERE s.tenant_id = $3 AND en.class_id = $5
                  AND (en.status = 'Active' OR en.status = 'ACTIVE')
                "#,
            )
            .bind(&title_c)
            .bind(&body_c)
            .bind(tid)
            .bind(final_session_id)
            .bind(class_id)
            .execute(&ctx.pool)
            .await;
            crate::infrastructure::fcm::trigger_fcm_push_categorized(
                t,
                b,
                crate::infrastructure::fcm::FcmCategory::Session,
                final_session_id,
            );
        }

        let full_session = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, final_session_id).await
            .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

        return Ok(Json(ApiResponse::success(full_session, req_ctx.request_id)));
    }

    // Kasus 2: Sesi Ad-hoc (Tanpa Jadwal Template)
    let class_id = payload.class_id.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "class_id wajib diisi untuk sesi pembelajaran ad-hoc".to_string(),
                ),
            ),
            &req_ctx.request_id,
        )
    })?;

    let subject_id = payload.subject_id.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "subject_id wajib diisi untuk sesi pembelajaran ad-hoc".to_string(),
                ),
            ),
            &req_ctx.request_id,
        )
    })?;

    let teacher_id = payload
        .teacher_id
        .or(resolved_actor_teacher_id)
        .ok_or_else(|| {
            ApiError::new(
                school_core::common::error::ApplicationError::Domain(
                    school_core::common::error::DomainError::Validation(
                        "teacher_id wajib diisi atau harus terdaftar sebagai profil guru".to_string(),
                    ),
                ),
                &req_ctx.request_id,
            )
        })?;

    let session_number = payload.session_number.unwrap_or(1);
    let start_time_parsed = payload.start_time.as_deref().and_then(parse_naive_time);
    let end_time_parsed = payload.end_time.as_deref().and_then(parse_naive_time);

    let scheduled_at = start_time_parsed.and_then(|st| {
        session_date.and_time(st).and_local_timezone(Utc).single()
    });

    let new_session_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO learning_sessions (
            id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
            substitute_teacher_id, session_date, session_number, start_time, end_time,
            scheduled_at, started_at, status, notes, created_at, updated_at
        )
        VALUES ($1, $2, 'adhoc', NULL, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), 'active', $13, NOW(), NOW())
        "#
    )
    .bind(new_session_id)
    .bind(req_ctx.tenant_id)
    .bind(payload.lesson_id)
    .bind(class_id)
    .bind(subject_id)
    .bind(teacher_id)
    .bind(payload.substitute_teacher_id)
    .bind(session_date)
    .bind(session_number)
    .bind(start_time_parsed)
    .bind(end_time_parsed)
    .bind(scheduled_at)
    .bind(&payload.notes)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let full_session = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, new_session_id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(full_session, req_ctx.request_id)))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    Query(filter): Query<SessionFilterQuery>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SessionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;

    require_permission(&req_ctx.actor, Permission::LearningSessionRead)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .or_else(|_| {
            if req_ctx.actor.is_some() {
                Ok(())
            } else {
                Err(axum::http::StatusCode::UNAUTHORIZED)
            }
        })
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to read learning sessions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_management = req_ctx
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

    let resolved_actor_teacher_id = if let Some(uid) = user_id {
        crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
            &ctx.pool,
            req_ctx.tenant_id,
            uid,
        )
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let is_teacher = !is_management && (
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

    let is_student = !is_management && !is_teacher && req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n == "siswa" || n.contains("student") || n == "murid"
            })
        })
        .unwrap_or(false);

    let student_class_id: Option<Uuid> = if is_student {
        if let Some(uid) = user_id {
            sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT e.class_id 
                FROM enrollments e 
                JOIN students s ON s.id = e.student_id 
                WHERE s.user_id = $1 AND e.status ILIKE 'active'
                LIMIT 1
                "#,
            )
            .bind(uid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let target_class_id: Option<Uuid> = filter.class_id.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Uuid::parse_str(trimmed).ok()
        }
    });

    // Otomatis sinkronisasi sesi pembelajaran dari jadwal sekolah (class_schedules).
    // Kepala Sekolah, Guru, dan Siswa tidak perlu mengklik tombol "Mulai Sesi" secara manual
    // agar sesi hari ini otomatis tercipta dan berstatus sesuai jam operasional.
    let target_sync_date = filter.date.or(filter.from_date);
    let sync_sql = r#"
        INSERT INTO learning_sessions (
            id, tenant_id, session_type, schedule_id, class_id, subject_id, teacher_id,
            session_date, start_time, end_time, scheduled_at, status, created_at, updated_at
        )
        SELECT 
            gen_random_uuid(),
            cs.tenant_id,
            'scheduled',
            cs.id,
            cs.class_id,
            cs.subject_id,
            cs.teacher_id,
            d::date,
            cs.start_time::time,
            cs.end_time::time,
            (d::date + cs.start_time::time) AT TIME ZONE 'Asia/Jakarta',
            CASE 
                WHEN d::date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date THEN 'completed'
                WHEN d::date > (NOW() AT TIME ZONE 'Asia/Jakarta')::date THEN 'scheduled'
                WHEN (NOW() AT TIME ZONE 'Asia/Jakarta')::time > cs.end_time::time THEN 'completed'
                WHEN (NOW() AT TIME ZONE 'Asia/Jakarta')::time >= cs.start_time::time THEN 'active'
                ELSE 'scheduled'
            END,
            NOW(),
            NOW()
        FROM generate_series(
            date_trunc('week', COALESCE($2::date, (NOW() AT TIME ZONE 'Asia/Jakarta')::date))::date,
            (date_trunc('week', COALESCE($2::date, (NOW() AT TIME ZONE 'Asia/Jakarta')::date))::date + 5),
            interval '1 day'
        ) d
        JOIN class_schedules cs ON cs.day_of_week = CASE EXTRACT(DOW FROM d)::integer
            WHEN 0 THEN 'Minggu'
            WHEN 1 THEN 'Senin'
            WHEN 2 THEN 'Selasa'
            WHEN 3 THEN 'Rabu'
            WHEN 4 THEN 'Kamis'
            WHEN 5 THEN 'Jumat'
            WHEN 6 THEN 'Sabtu'
        END
        WHERE cs.tenant_id = $1
          AND cs.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1 FROM learning_sessions ls 
            WHERE ls.tenant_id = cs.tenant_id 
              AND ls.schedule_id = cs.id 
              AND ls.session_date = d::date
              AND ls.deleted_at IS NULL
          )
    "#;

    let _ = sqlx::query(sync_sql)
        .bind(req_ctx.tenant_id)
        .bind(target_sync_date)
        .execute(&ctx.pool)
        .await;

    // Sinkronisasi status riil terhadap jam sekarang untuk sesi pekan ini
    let update_status_sql = r#"
        UPDATE learning_sessions ls
        SET status = CASE 
                WHEN ls.session_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date THEN 'completed'
                WHEN ls.session_date > (NOW() AT TIME ZONE 'Asia/Jakarta')::date THEN 'scheduled'
                WHEN ls.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time > ls.end_time THEN 'completed'
                WHEN ls.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time >= ls.start_time THEN 'active'
                ELSE ls.status
            END,
            updated_at = NOW()
        WHERE ls.tenant_id = $1
          AND ls.session_date BETWEEN date_trunc('week', COALESCE($2::date, (NOW() AT TIME ZONE 'Asia/Jakarta')::date))::date
                                  AND (date_trunc('week', COALESCE($2::date, (NOW() AT TIME ZONE 'Asia/Jakarta')::date))::date + 5)
          AND ls.status NOT IN ('cancelled')
          AND ls.deleted_at IS NULL
          AND (
              (ls.session_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND ls.status != 'completed')
              OR (ls.session_date > (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND ls.status != 'scheduled')
              OR (ls.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time > ls.end_time AND ls.status != 'completed')
              OR (ls.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time >= ls.start_time AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time <= ls.end_time AND ls.status != 'active')
          )
    "#;

    let _ = sqlx::query(update_status_sql)
        .bind(req_ctx.tenant_id)
        .bind(target_sync_date)
        .execute(&ctx.pool)
        .await;

    let rows = sqlx::query(
        r#"
        SELECT 
            ls.id,
            ls.tenant_id,
            ls.session_type,
            ls.schedule_id,
            ls.lesson_id,
            ls.class_id,
            ls.subject_id,
            ls.teacher_id,
            ls.substitute_teacher_id,
            ls.session_date,
            ls.session_number,
            ls.start_time,
            ls.end_time,
            ls.scheduled_at,
            ls.started_at,
            ls.ended_at,
            ls.status,
            ls.notes,
            ls.cancellation_reason,
            ls.created_at,
            ls.updated_at,
            c.name as class_name,
            s.name as subject_name,
            t.full_name as teacher_name,
            st.full_name as substitute_teacher_name,
            COALESCE(cs.room, 'Ruang Kelas') as room
        FROM learning_sessions ls
        LEFT JOIN classes c ON c.id = ls.class_id
        LEFT JOIN subjects s ON s.id = ls.subject_id
        LEFT JOIN teachers t ON t.id = ls.teacher_id
        LEFT JOIN teachers st ON st.id = ls.substitute_teacher_id
        LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
        WHERE ls.tenant_id = $1 
          AND ls.deleted_at IS NULL
          AND ($2::uuid IS NULL OR ls.class_id = $2)
          AND ($3::uuid IS NULL OR ls.teacher_id = $3 OR ls.substitute_teacher_id = $3)
          AND ($4::uuid IS NULL OR ls.schedule_id = $4)
          AND ($5::date IS NULL OR ls.session_date = $5)
          AND ($6::date IS NULL OR ls.session_date >= $6)
          AND ($7::date IS NULL OR ls.session_date <= $7)
          AND ($8::text IS NULL OR ls.status = $8)
          AND (
              -- Student filter: only enrolled class
              CASE WHEN $9::boolean THEN ls.class_id = $10::uuid ELSE TRUE END
          )
          AND (
              -- Teacher filter: only taught sessions
              CASE WHEN $11::boolean THEN (ls.teacher_id = $12::uuid OR ls.substitute_teacher_id = $12::uuid) ELSE TRUE END
          )
        ORDER BY ls.session_date DESC, ls.start_time DESC NULLS LAST, ls.created_at DESC
        LIMIT 200
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(target_class_id)
    .bind(filter.teacher_id)
    .bind(filter.schedule_id)
    .bind(filter.date)
    .bind(filter.from_date)
    .bind(filter.to_date)
    .bind(filter.status)
    .bind(is_student)
    .bind(student_class_id)
    .bind(is_teacher && !is_management)
    .bind(resolved_actor_teacher_id)
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

    let items: Vec<SessionResponse> = rows.into_iter().map(|r| {
        let st: Option<NaiveTime> = r.get("start_time");
        let et: Option<NaiveTime> = r.get("end_time");

        SessionResponse {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            session_type: r.get::<Option<String>, _>("session_type").unwrap_or_else(|| "scheduled".to_string()),
            schedule_id: r.get("schedule_id"),
            lesson_id: r.get("lesson_id"),
            class_id: r.get("class_id"),
            subject_id: r.get("subject_id"),
            teacher_id: r.get("teacher_id"),
            substitute_teacher_id: r.get("substitute_teacher_id"),
            session_date: r.get::<Option<NaiveDate>, _>("session_date").unwrap_or_else(|| chrono::Utc::now().date_naive()),
            session_number: r.get::<Option<i32>, _>("session_number").unwrap_or(1),
            start_time: st.map(|t| t.format("%H:%M").to_string()),
            end_time: et.map(|t| t.format("%H:%M").to_string()),
            scheduled_at: r.get("scheduled_at"),
            started_at: r.get("started_at"),
            ended_at: r.get("ended_at"),
            status: r.get("status"),
            notes: r.get("notes"),
            cancellation_reason: r.get("cancellation_reason"),
            subject_name: r.get("subject_name"),
            teacher_name: r.get("teacher_name"),
            substitute_teacher_name: r.get("substitute_teacher_name"),
            class_name: r.get("class_name"),
            room: r.get("room"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;

    require_permission(&req_ctx.actor, Permission::LearningSessionRead)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .or_else(|_| {
            if req_ctx.actor.is_some() {
                Ok(())
            } else {
                Err(axum::http::StatusCode::UNAUTHORIZED)
            }
        })
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    // Cari dari database session aktual
    if let Ok(res) = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, id).await {
        return Ok(Json(ApiResponse::success(res, req_ctx.request_id)));
    }

    // Graceful resolution: jika ID ternyata adalah schedule_id hari ini, coba cari atau inisialisasi sesi hari ini
    let today = Utc::now().date_naive();
    if let Ok(existing_by_sched) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_sessions WHERE tenant_id = $1 AND schedule_id = $2 AND session_date = $3 AND deleted_at IS NULL LIMIT 1"
    )
    .bind(req_ctx.tenant_id)
    .bind(id)
    .bind(today)
    .fetch_one(&ctx.pool)
    .await {
        if let Ok(res) = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, existing_by_sched).await {
            return Ok(Json(ApiResponse::success(res, req_ctx.request_id)));
        }
    }

    Err(ApiError::new(
        school_core::common::error::ApplicationError::NotFound(
            school_core::common::error_code::ErrorCode::SessionNotFound,
            format!("Sesi pembelajaran dengan ID {} tidak ditemukan", id),
        ),
        &req_ctx.request_id,
    ))
}

async fn end(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to end session".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let command = EndSessionCommand { session_id: id };

    let session = ctx
        .end_session
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let full_session = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, session.id)
        .await
        .unwrap_or_else(|_| SessionResponse::from(session));

    Ok(Json(ApiResponse::success(full_session, req_ctx.request_id)))
}

async fn cancel_session(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelSessionPayload>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to cancel session".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    sqlx::query(
        r#"
        UPDATE learning_sessions
        SET status = 'cancelled',
            cancellation_reason = $1,
            updated_at = NOW()
        WHERE id = $2 AND tenant_id = $3 AND deleted_at IS NULL
        "#
    )
    .bind(&payload.reason)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let full_session = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(full_session, req_ctx.request_id)))
}

async fn substitute_teacher(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubstituteTeacherPayload>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to assign substitute teacher".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    // Validasi guru pengganti berada di tenant yang sama
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM teachers WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL)"
    )
    .bind(payload.substitute_teacher_id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(false);

    if !exists {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::TeacherNotFound,
                "Guru pengganti tidak ditemukan pada tenant ini".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    sqlx::query(
        r#"
        UPDATE learning_sessions
        SET substitute_teacher_id = $1,
            notes = COALESCE($2, notes),
            updated_at = NOW()
        WHERE id = $3 AND tenant_id = $4 AND deleted_at IS NULL
        "#
    )
    .bind(payload.substitute_teacher_id)
    .bind(&payload.notes)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let full_session = fetch_full_session_dto(&ctx.pool, req_ctx.tenant_id, id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(full_session, req_ctx.request_id)))
}

async fn record_attendance(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<RecordAttendanceRequest>,
) -> Result<Json<ApiResponse<AttendanceResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to record attendance".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let target_session_id = resolve_effective_session_id(&ctx.pool, req_ctx.tenant_id, id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let recorded_by = req_ctx.actor.as_ref().map(|a| a.id);

    let command = RecordAttendanceCommand {
        tenant_id: req_ctx.tenant_id,
        session_id: target_session_id,
        student_id: payload.student_id,
        status: payload.status,
        checked_in_at: payload.checked_in_at,
        notes: payload.notes,
        recorded_by,
    };

    let attendance = ctx
        .record_attendance
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttendanceResponse::from(attendance),
        req_ctx.request_id,
    )))
}

async fn get_attendance(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<AttendanceResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionRead)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .or_else(|_| {
            if req_ctx.actor.is_some() {
                Ok(())
            } else {
                Err(axum::http::StatusCode::UNAUTHORIZED)
            }
        })
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let target_session_id = resolve_effective_session_id(&ctx.pool, req_ctx.tenant_id, id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let query = GetAttendanceQuery {
        tenant_id: req_ctx.tenant_id,
        session_id: target_session_id,
    };

    let records = ctx
        .get_attendance
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = records.into_iter().map(AttendanceResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn record_attendance_bulk(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<Vec<AttendanceItemDto>>,
) -> Result<Json<ApiResponse<Vec<AttendanceResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let target_session_id = resolve_effective_session_id(&ctx.pool, req_ctx.tenant_id, id).await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let recorded_by = req_ctx.actor.as_ref().map(|a| a.id);

    let command = RecordAttendanceBulkCommand {
        tenant_id: req_ctx.tenant_id,
        session_id: target_session_id,
        items: payload,
        method: Some("manual".to_string()),
        recorded_by,
    };

    let attendances = ctx
        .record_attendance_bulk
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = attendances.into_iter().map(AttendanceResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

/// Helper untuk fetch SessionResponse lengkap dengan relasi kelas, mapel, guru, dan ruang
async fn fetch_full_session_dto(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<SessionResponse, school_core::common::error::ApplicationError> {
    let row = sqlx::query(
        r#"
        SELECT 
            ls.id,
            ls.tenant_id,
            ls.session_type,
            ls.schedule_id,
            ls.lesson_id,
            ls.class_id,
            ls.subject_id,
            ls.teacher_id,
            ls.substitute_teacher_id,
            ls.session_date,
            ls.session_number,
            ls.start_time,
            ls.end_time,
            ls.scheduled_at,
            ls.started_at,
            ls.ended_at,
            ls.status,
            ls.notes,
            ls.cancellation_reason,
            ls.created_at,
            ls.updated_at,
            c.name as class_name,
            s.name as subject_name,
            t.full_name as teacher_name,
            st.full_name as substitute_teacher_name,
            COALESCE(cs.room, 'Ruang Kelas') as room
        FROM learning_sessions ls
        LEFT JOIN classes c ON c.id = ls.class_id
        LEFT JOIN subjects s ON s.id = ls.subject_id
        LEFT JOIN teachers t ON t.id = ls.teacher_id
        LEFT JOIN teachers st ON st.id = ls.substitute_teacher_id
        LEFT JOIN class_schedules cs ON cs.id = ls.schedule_id
        WHERE ls.id = $1 AND ls.tenant_id = $2 AND ls.deleted_at IS NULL
        "#
    )
    .bind(session_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e),
    ))?
    .ok_or_else(|| school_core::common::error::ApplicationError::NotFound(
        school_core::common::error_code::ErrorCode::SessionNotFound,
        format!("Sesi {} tidak ditemukan", session_id),
    ))?;

    let st: Option<NaiveTime> = row.get("start_time");
    let et: Option<NaiveTime> = row.get("end_time");

    Ok(SessionResponse {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        session_type: row.get::<Option<String>, _>("session_type").unwrap_or_else(|| "scheduled".to_string()),
        schedule_id: row.get("schedule_id"),
        lesson_id: row.get("lesson_id"),
        class_id: row.get("class_id"),
        subject_id: row.get("subject_id"),
        teacher_id: row.get("teacher_id"),
        substitute_teacher_id: row.get("substitute_teacher_id"),
        session_date: row.get::<Option<NaiveDate>, _>("session_date").unwrap_or_else(|| chrono::Utc::now().date_naive()),
        session_number: row.get::<Option<i32>, _>("session_number").unwrap_or(1),
        start_time: st.map(|t| t.format("%H:%M").to_string()),
        end_time: et.map(|t| t.format("%H:%M").to_string()),
        scheduled_at: row.get("scheduled_at"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        status: row.get("status"),
        notes: row.get("notes"),
        cancellation_reason: row.get("cancellation_reason"),
        subject_name: row.get("subject_name"),
        teacher_name: row.get("teacher_name"),
        substitute_teacher_name: row.get("substitute_teacher_name"),
        class_name: row.get("class_name"),
        room: row.get("room"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

/// Helper toleran untuk presensi: jika frontend lama mengirim schedule_id, temukan atau buat sesi aktual hari ini
async fn resolve_effective_session_id(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Uuid, school_core::common::error::ApplicationError> {
    // 1. Apakah id adalah learning_sessions.id?
    let session_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM learning_sessions WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL)"
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if session_exists {
        return Ok(id);
    }

    // 2. Jika bukan, apakah id adalah class_schedules.id?
    let sched_row = sqlx::query(
        r#"
        SELECT id, class_id, subject_id, teacher_id, start_time, end_time
        FROM class_schedules
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e),
    ))?;

    if let Some(sched) = sched_row {
        let sched_id: Uuid = sched.try_get("id").unwrap_or_default();
        let sched_class_id: Uuid = sched.try_get("class_id").unwrap_or_default();
        let sched_subject_id: Uuid = sched.try_get("subject_id").unwrap_or_default();
        let sched_teacher_id: Uuid = sched.try_get("teacher_id").unwrap_or_default();
        let sched_start_time: String = sched.try_get("start_time").unwrap_or_default();
        let sched_end_time: String = sched.try_get("end_time").unwrap_or_default();

        let today = Utc::now().date_naive();
        // Cek apakah sesi hari ini sudah ada
        let existing = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM learning_sessions WHERE tenant_id = $1 AND schedule_id = $2 AND session_date = $3 AND deleted_at IS NULL LIMIT 1"
        )
        .bind(tenant_id)
        .bind(sched_id)
        .bind(today)
        .fetch_optional(pool)
        .await
        .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e),
        ))?;

        if let Some(sid) = existing {
            return Ok(sid);
        }

        // Buat sesi otomatis untuk hari ini
        let new_id = Uuid::new_v4();
        let st = parse_naive_time(&sched_start_time);
        let et = parse_naive_time(&sched_end_time);
        let scheduled_at = st.and_then(|t| today.and_time(t).and_local_timezone(Utc).single());

        let created_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO learning_sessions (
                id, tenant_id, session_type, schedule_id, lesson_id, class_id, subject_id, teacher_id,
                session_date, session_number, start_time, end_time, scheduled_at, started_at, status, created_at, updated_at
            )
            VALUES ($1, $2, 'scheduled', $3, NULL, $4, $5, $6, $7, 1, $8, $9, $10, NOW(), 'active', NOW(), NOW())
            ON CONFLICT (tenant_id, schedule_id, session_date)
            WHERE schedule_id IS NOT NULL AND deleted_at IS NULL
            DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#
        )
        .bind(new_id)
        .bind(tenant_id)
        .bind(sched_id)
        .bind(sched_class_id)
        .bind(sched_subject_id)
        .bind(sched_teacher_id)
        .bind(today)
        .bind(st)
        .bind(et)
        .bind(scheduled_at)
        .fetch_one(pool)
        .await
        .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e),
        ))?;

        return Ok(created_id);
    }

    Err(school_core::common::error::ApplicationError::NotFound(
        school_core::common::error_code::ErrorCode::SessionNotFound,
        format!("Sesi pembelajaran dengan ID {} tidak ditemukan", id),
    ))
}
