use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
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
    get_session::GetSessionQuery, list_sessions::ListSessionsQuery,
    record_attendance::RecordAttendanceCommand, start_session::StartSessionCommand,
};

pub fn session_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(start).get(list))
        .route("/{id}", get(get_by_id))
        .route("/{id}/end", post(end))
        .route(
            "/{id}/attendance",
            post(record_attendance).get(get_attendance),
        )
}

async fn start(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<StartSessionRequest>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = StartSessionCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        class_id: payload.class_id,
        teacher_id: payload.teacher_id,
        notes: payload.notes,
    };

    let session = ctx
        .start_session
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SessionResponse::from(session),
        req_ctx.request_id,
    )))
}

#[derive(Debug, Deserialize)]
pub struct SessionFilterQuery {
    pub class_id: Option<String>,
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
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let query = ListSessionsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let sessions = ctx
        .list_sessions
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let mut items: Vec<SessionResponse> = sessions.into_iter().map(SessionResponse::from).collect();

    if items.is_empty() {
        use chrono::Datelike;

        let schedule_rows = sqlx::query!(
            r#"
            SELECT 
                cs.id, cs.tenant_id, cs.class_id, cs.subject_id, cs.teacher_id,
                cs.day_of_week, cs.start_time, cs.end_time, COALESCE(cs.room, 'Ruang Kelas') as "room!",
                s.name as subject_name, t.full_name as teacher_name, t.user_id as teacher_user_id,
                c.name as class_name
            FROM class_schedules cs
            JOIN subjects s ON s.id = cs.subject_id
            JOIN teachers t ON t.id = cs.teacher_id
            JOIN classes c ON c.id = cs.class_id
            WHERE cs.tenant_id = $1 AND cs.deleted_at IS NULL
            "#,
            req_ctx.tenant_id
        )
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

        let now = chrono::Utc::now();
        let days_from_monday = now.weekday().num_days_from_monday() as i64;
        let monday = now.date_naive() - chrono::Duration::days(days_from_monday);

        let user_id = req_ctx.actor.as_ref().map(|a| a.id);
        let is_student = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Siswa" || r.name == "Student")).unwrap_or(false);
        let is_teacher = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Guru" || r.name == "Teacher")).unwrap_or(false);
        let is_management = req_ctx.actor.as_ref().map(|a| a.roles.iter().any(|r| r.name == "Administrator" || r.name == "Admin" || r.name == "Kepala Sekolah" || r.name == "Operator" || r.name == "Staf TU")).unwrap_or(true);

        let student_class_id: Option<Uuid> = if is_student {
            if let Some(uid) = user_id {
                sqlx::query_scalar!(
                    r#"
                    SELECT e.class_id 
                    FROM enrollments e 
                    JOIN students s ON s.id = e.student_id 
                    WHERE s.user_id = $1 AND e.status ILIKE 'active'
                    LIMIT 1
                    "#,
                    uid
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
        };

        let target_class_id: Option<Uuid> = filter.class_id
            .as_deref()
            .and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Uuid::parse_str(trimmed).ok()
                }
            });

        for row in schedule_rows {
            // Filter by explicit query class_id
            if let Some(target_cid) = target_class_id {
                if row.class_id != target_cid {
                    continue;
                }
            }

            // Student isolation: student only sees their own enrolled class schedule
            if is_student {
                match student_class_id {
                    Some(sc_id) => {
                        if row.class_id != sc_id {
                            continue;
                        }
                    }
                    None => {
                        // Student without an active enrolled class sees no schedule
                        continue;
                    }
                }
            }

            // Teacher isolation: non-management teachers only see their own teaching schedules
            // IMPORTANT: If teacher_user_id is NULL (teacher has no linked user), skip the row
            // to prevent data leakage between teachers.
            if is_teacher && !is_management {
                match user_id {
                    Some(uid) => {
                        match row.teacher_user_id {
                            Some(t_uid) if t_uid == uid => {
                                // This schedule belongs to the logged-in teacher — allow it
                            }
                            _ => {
                                // NULL teacher_user_id or different teacher — skip to prevent leakage
                                continue;
                            }
                        }
                    }
                    None => {
                        // No user context for teacher — skip all to prevent leakage
                        continue;
                    }
                }
            }

            let day_offset = match row.day_of_week.to_lowercase().as_str() {
                "senin" => 0,
                "selasa" => 1,
                "rabu" => 2,
                "kamis" => 3,
                "jumat" => 4,
                "sabtu" => 5,
                _ => 6,
            };

            let target_date = monday + chrono::Duration::days(day_offset);

            let start_parts: Vec<&str> = row.start_time.split(':').collect();
            let start_h: u32 = start_parts.first().and_then(|s| s.parse().ok()).unwrap_or(8);
            let start_m: u32 = start_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

            let end_parts: Vec<&str> = row.end_time.split(':').collect();
            let end_h: u32 = end_parts.first().and_then(|s| s.parse().ok()).unwrap_or(9);
            let end_m: u32 = end_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);

            let start_naive = target_date.and_hms_opt(start_h, start_m, 0);
            let end_naive = target_date.and_hms_opt(end_h, end_m, 0);

            let scheduled_at = start_naive.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt - chrono::Duration::hours(7), chrono::Utc));
            let ended_at = end_naive.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt - chrono::Duration::hours(7), chrono::Utc));

            let status = if let (Some(s), Some(e)) = (scheduled_at, ended_at) {
                if now >= s && now <= e {
                    "active".to_string()
                } else if now > e {
                    "completed".to_string()
                } else {
                    "scheduled".to_string()
                }
            } else {
                "scheduled".to_string()
            };

            items.push(SessionResponse {
                id: row.id,
                tenant_id: row.tenant_id,
                lesson_id: row.subject_id,
                class_id: row.class_id,
                teacher_id: row.teacher_id,
                scheduled_at,
                started_at: if status == "active" || status == "completed" { scheduled_at } else { None },
                ended_at: if status == "completed" { ended_at } else { None },
                status,
                notes: Some(format!("{} • {}", row.subject_name, row.room)),
                subject_name: Some(row.subject_name),
                teacher_name: Some(row.teacher_name),
                class_name: Some(row.class_name),
                room: Some(row.room),
                created_at: now,
                updated_at: now,
            });
        }
    }

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

    let query = GetSessionQuery { session_id: id };

    if let Ok(session) = ctx.get_session.execute(query).await {
        return Ok(Json(ApiResponse::success(
            SessionResponse::from(session),
            req_ctx.request_id,
        )));
    }

    // Fallback to query class_schedules by id
    use chrono::Datelike;
    let schedule_row = sqlx::query!(
        r#"
        SELECT 
            cs.id, cs.tenant_id, cs.class_id, cs.subject_id, cs.teacher_id,
            cs.day_of_week, cs.start_time, cs.end_time, COALESCE(cs.room, 'Ruang Kelas') as "room!",
            s.name as subject_name, t.full_name as teacher_name, t.user_id as teacher_user_id,
            c.name as class_name
        FROM class_schedules cs
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        JOIN classes c ON c.id = cs.class_id
        WHERE cs.id = $1 AND cs.tenant_id = $2 AND cs.deleted_at IS NULL
        "#,
        id,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Internal(e.to_string()), &req_ctx.request_id))?;

    if let Some(row) = schedule_row {
        let now = chrono::Utc::now();
        let days_from_monday = now.weekday().num_days_from_monday() as i64;
        let monday = now.date_naive() - chrono::Duration::days(days_from_monday);

        let day_offset = match row.day_of_week.to_lowercase().as_str() {
            "senin" => 0,
            "selasa" => 1,
            "rabu" => 2,
            "kamis" => 3,
            "jumat" => 4,
            "sabtu" => 5,
            _ => 6,
        };

        let target_date = monday + chrono::Duration::days(day_offset);

        let start_parts: Vec<&str> = row.start_time.split(':').collect();
        let start_h: u32 = start_parts.first().and_then(|s| s.parse().ok()).unwrap_or(8);
        let start_m: u32 = start_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        let end_parts: Vec<&str> = row.end_time.split(':').collect();
        let end_h: u32 = end_parts.first().and_then(|s| s.parse().ok()).unwrap_or(9);
        let end_m: u32 = end_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);

        let start_naive = target_date.and_hms_opt(start_h, start_m, 0);
        let end_naive = target_date.and_hms_opt(end_h, end_m, 0);

        let scheduled_at = start_naive.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt - chrono::Duration::hours(7), chrono::Utc));
        let ended_at = end_naive.map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt - chrono::Duration::hours(7), chrono::Utc));

        let status = if let (Some(s), Some(e)) = (scheduled_at, ended_at) {
            if now >= s && now <= e {
                "active".to_string()
            } else if now > e {
                "completed".to_string()
            } else {
                "scheduled".to_string()
            }
        } else {
            "scheduled".to_string()
        };

        let res = SessionResponse {
            id: row.id,
            tenant_id: row.tenant_id,
            lesson_id: row.subject_id,
            class_id: row.class_id,
            teacher_id: row.teacher_id,
            scheduled_at,
            started_at: if status == "active" || status == "completed" { scheduled_at } else { None },
            ended_at: if status == "completed" { ended_at } else { None },
            status,
            notes: Some(format!("{} • {}", row.subject_name, row.room)),
            subject_name: Some(row.subject_name),
            teacher_name: Some(row.teacher_name),
            class_name: Some(row.class_name),
            room: Some(row.room),
            created_at: now,
            updated_at: now,
        };

        return Ok(Json(ApiResponse::success(res, req_ctx.request_id)));
    }

    Err(ApiError::new(
        school_core::common::error::ApplicationError::NotFound(
            school_core::common::error_code::ErrorCode::SessionNotFound,
            format!("Session {} not found", id),
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
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
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

    Ok(Json(ApiResponse::success(
        SessionResponse::from(session),
        req_ctx.request_id,
    )))
}

async fn record_attendance(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<RecordAttendanceRequest>,
) -> Result<Json<ApiResponse<AttendanceResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = RecordAttendanceCommand {
        tenant_id: req_ctx.tenant_id,
        session_id: id,
        student_id: payload.student_id,
        status: payload.status,
        checked_in_at: payload.checked_in_at,
        notes: payload.notes,
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

    let query = GetAttendanceQuery { session_id: id };

    let records = ctx
        .get_attendance
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = records.into_iter().map(AttendanceResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}
