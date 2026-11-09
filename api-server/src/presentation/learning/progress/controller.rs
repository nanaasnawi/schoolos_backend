use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Duration, FixedOffset, Utc};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    calculate_progress_request::CalculateProgressRequest,
    progress_response::ProgressResponse,
    study_tracker_response::{StudyTrackerResponse, TrackerCategory, TrackerItem},
    weekly_activity_response::{DailyActivityItem, DailyActivityStat, WeeklyActivityResponse},
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::progress::{
    calculate_progress::CalculateProgressCommand, get_progress::GetProgressQuery,
};

#[derive(Debug, Deserialize)]
pub struct WeeklyActivityQuery {
    pub student_id: Option<Uuid>,
}

pub fn progress_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/calculate", post(calculate))
        .route("/me", get(get_my_progress))
        .route("/weekly-activity", get(get_weekly_activity))
        .route("/tracker", get(get_my_study_tracker))
        .route("/student/{student_id}", get(get_student_progress_by_id))
        .route("/student/{student_id}/weekly-activity", get(get_student_weekly_activity_by_id))
        .route("/student/{student_id}/tracker", get(get_student_study_tracker_by_id))
        .route("/{student_id}/{class_id}/{subject_id}", get(get_progress))
}

async fn get_my_progress(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthTokenExpired,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Find student id & active class id
    let student_info = sqlx::query!(
        r#"
        SELECT s.id as student_id, en.class_id
        FROM students s
        LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
        WHERE s.user_id = $1 AND s.tenant_id = $2
        LIMIT 1
        "#,
        actor_id,
        req_ctx.tenant_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let (student_id, class_id) = match student_info {
        Some(s) => (s.student_id, s.class_id),
        None => {
            // Check if guardian/parent has student
            let child = sqlx::query!(
                r#"
                SELECT s.id as student_id, en.class_id
                FROM guardians g
                JOIN students s ON s.guardian_id = g.id
                LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
                WHERE g.user_id = $1 AND g.tenant_id = $2
                LIMIT 1
                "#,
                actor_id,
                req_ctx.tenant_id
            )
            .fetch_optional(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e)
            ), &req_ctx.request_id))?;

            match child {
                Some(c) => (c.student_id, c.class_id),
                None => {
                    return Err(ApiError::new(
                        school_core::common::error::ApplicationError::NotFound(
                            school_core::common::error_code::ErrorCode::ResourceNotFound,
                            "Data siswa tidak ditemukan untuk akun ini".to_string(),
                        ),
                        &req_ctx.request_id,
                    ));
                }
            }
        }
    };

    let resp =
        calculate_dynamic_student_progress(&ctx, req_ctx.tenant_id, student_id, Some(class_id))
            .await
            .map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Infrastructure(
                        school_core::common::error::InfrastructureError::Database(e),
                    ),
                    &req_ctx.request_id,
                )
            })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_student_progress_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    let class_id = sqlx::query_scalar!(
        r#"
        SELECT en.class_id
        FROM enrollments en
        WHERE en.student_id = $1 AND (en.status = 'Active' OR en.status = 'ACTIVE')
        LIMIT 1
        "#,
        student_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let resp = calculate_dynamic_student_progress(&ctx, req_ctx.tenant_id, student_id, class_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn calculate_dynamic_student_progress(
    ctx: &ApplicationContext,
    tenant_id: Uuid,
    student_id: Uuid,
    class_id: Option<Uuid>,
) -> Result<ProgressResponse, sqlx::Error> {
    // 1. Learning materials
    let lesson_total: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM learning_materials
        WHERE tenant_id = $1 AND deleted_at IS NULL AND is_active = true
          AND (class_id IS NULL OR class_id = $2)
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let lesson_completed: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM student_material_completions
        WHERE tenant_id = $1 AND student_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 2. Assignments
    let assignment_total: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM assignments
        WHERE tenant_id = $1 AND deleted_at IS NULL
          AND (class_id IS NULL OR class_id = $2)
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let assignment_completed: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT assignment_id)::int
        FROM assignment_submissions
        WHERE student_id = $1 AND (status = 'submitted' OR status = 'graded')
        "#,
    )
    .bind(student_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 3. Quizzes
    let quiz_total: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM quizzes
        WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let quiz_completed: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT quiz_id)::int
        FROM quiz_attempts
        WHERE student_id = $1
        "#,
    )
    .bind(student_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 4. Sessions
    let session_total: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM learning_sessions
        WHERE tenant_id = $1 AND (class_id IS NULL OR class_id = $2) AND status = 'completed' AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let session_attended: i32 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::int
        FROM session_attendances
        WHERE tenant_id = $1 AND student_id = $2 AND (status = 'present' OR status = 'late')
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // Calculate overall progress percentage
    let total_deliverables = lesson_total + assignment_total;
    let completed_deliverables = lesson_completed + assignment_completed;
    let overall_progress = if total_deliverables > 0 {
        (((completed_deliverables as f64) / (total_deliverables as f64)) * 100.0 * 10.0).round()
            / 10.0
    } else {
        0.0
    };

    // Query student name
    let student_name: String = sqlx::query_scalar(
        r#"SELECT full_name FROM students WHERE id = $1 LIMIT 1"#,
    )
    .bind(student_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Siswa".to_string());

    // Query class name and homeroom teacher name
    let (class_name, homeroom_teacher_name) = if let Some(cid) = class_id {
        let r = sqlx::query(
            r#"
            SELECT c.name as class_name, t.full_name as teacher_name
            FROM classes c
            LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id
            WHERE c.id = $1
            LIMIT 1
            "#,
        )
        .bind(cid)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        match r {
            Some(row) => (
                row.try_get::<String, _>("class_name").ok(),
                row.try_get::<Option<String>, _>("teacher_name").ok().flatten(),
            ),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    let missing_assignments = (assignment_total - assignment_completed).max(0);

    let academic_status = if overall_progress >= 85.0 {
        "Sangat Baik"
    } else if overall_progress >= 70.0 {
        "Baik"
    } else if overall_progress >= 50.0 {
        "Cukup"
    } else if missing_assignments > 0 {
        "Perlu Perhatian"
    } else {
        "Perlu Bimbingan"
    };

    let class_display_name = class_name.as_deref().unwrap_or("Kelas");

    let teacher_notes = if missing_assignments > 0 && session_attended > 0 {
        format!(
            "{} memiliki catatan kehadiran yang baik ({} sesi diikuti), namun terdapat {} tugas aktif yang belum dikumpulkan. Mohon bimbingan dan pendampingan orang tua di rumah agar tugas dapat segera diselesaikan.",
            student_name, session_attended, missing_assignments
        )
    } else if missing_assignments == 0
        && total_deliverables > 0
        && completed_deliverables == total_deliverables
    {
        format!(
            "Luar biasa! Seluruh modul pembelajaran dan penugasan telah diselesaikan oleh {} dengan tepat waktu. Pertahankan prestasi dan dedikasi belajar ini.",
            student_name
        )
    } else if missing_assignments > 0 {
        format!(
            "Terdapat {} penugasan yang masih perlu diselesaikan oleh {}. Disarankan untuk menyusun jadwal belajar mandiri di rumah dengan pendampingan orang tua.",
            missing_assignments, student_name
        )
    } else if lesson_completed > 0 {
        format!(
            "{} aktif mempelajari materi modul ({} materi tuntas). Keaktifan dan pemahaman materi terus menunjukkan tren yang sangat positif di kelas.",
            student_name, lesson_completed
        )
    } else {
        format!(
            "Semester ini telah dimulai. Diharapkan orang tua memotivasi {} untuk mulai mempelajari materi modul dan aktif mengikuti agenda kegiatan di {}.",
            student_name, class_display_name
        )
    };

    Ok(ProgressResponse {
        id: student_id,
        student_id,
        class_id: class_id.unwrap_or_else(Uuid::nil),
        subject_id: Uuid::nil(),
        overall_progress,
        lesson_completed,
        lesson_total,
        assignment_completed,
        assignment_total,
        quiz_completed,
        quiz_total,
        session_attended,
        session_total,
        calculated_at: Utc::now(),
        teacher_notes: Some(teacher_notes),
        teacher_name: homeroom_teacher_name,
        class_name,
        academic_status: Some(academic_status.to_string()),
    })
}

async fn calculate(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CalculateProgressRequest>,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningProgressUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CalculateProgressCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: payload.student_id,
        class_id: payload.class_id,
    };

    let progress = ctx
        .calculate_progress
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        ProgressResponse::from(progress),
        req_ctx.request_id,
    )))
}

async fn get_progress(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((student_id, class_id, subject_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<ApiResponse<ProgressResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningProgressRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetProgressQuery {
        student_id,
        class_id,
        subject_id,
    };

    let progress_res = ctx.get_progress.execute(query).await;

    match progress_res {
        Ok(progress) => Ok(Json(ApiResponse::success(
            ProgressResponse::from(progress),
            req_ctx.request_id,
        ))),
        Err(_) => {
            // Fallback to dynamic calculation
            let dynamic = calculate_dynamic_student_progress(
                &ctx,
                req_ctx.tenant_id,
                student_id,
                Some(class_id),
            )
            .await
            .map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Infrastructure(
                        school_core::common::error::InfrastructureError::Database(e),
                    ),
                    &req_ctx.request_id,
                )
            })?;
            Ok(Json(ApiResponse::success(dynamic, req_ctx.request_id)))
        }
    }
}

async fn get_weekly_activity(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<WeeklyActivityQuery>,
) -> Result<Json<ApiResponse<WeeklyActivityResponse>>, ApiError> {
    let student_id = if let Some(sid) = query.student_id {
        sid
    } else {
        let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthTokenExpired,
                    "Authentication required".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

        // Find student directly from user_id
        let student_row = sqlx::query(
            r#"
            SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1
            "#,
        )
        .bind(actor_id)
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

        if let Some(s) = student_row {
            s.get::<Uuid, _>("id")
        } else {
            // Check guardian
            let child_row = sqlx::query(
                r#"
                SELECT s.id
                FROM guardians g
                JOIN students s ON s.guardian_id = g.id
                WHERE g.user_id = $1 AND g.tenant_id = $2
                LIMIT 1
                "#,
            )
            .bind(actor_id)
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

            if let Some(c) = child_row {
                c.get::<Uuid, _>("id")
            } else {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::NotFound(
                        school_core::common::error_code::ErrorCode::ResourceNotFound,
                        "Data siswa tidak ditemukan untuk akun ini".to_string(),
                    ),
                    &req_ctx.request_id,
                ));
            }
        }
    };

    let resp = calculate_student_weekly_activity(&ctx, req_ctx.tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_student_weekly_activity_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
) -> Result<Json<ApiResponse<WeeklyActivityResponse>>, ApiError> {
    let resp = calculate_student_weekly_activity(&ctx, req_ctx.tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn calculate_student_weekly_activity(
    ctx: &ApplicationContext,
    tenant_id: Uuid,
    student_id: Uuid,
) -> Result<WeeklyActivityResponse, sqlx::Error> {
    // 1. Get student name
    let student_name_row = sqlx::query(
        r#"
        SELECT COALESCE(s.full_name, u.full_name, 'Siswa') as student_name
        FROM students s
        LEFT JOIN users u ON u.id = s.user_id
        WHERE s.id = $1 AND s.tenant_id = $2
        "#,
    )
    .bind(student_id)
    .bind(tenant_id)
    .fetch_optional(&ctx.pool)
    .await?;

    let student_name: String = student_name_row
        .and_then(|r| r.get::<Option<String>, _>("student_name"))
        .unwrap_or_else(|| "Siswa".to_string());

    // 2. Week bounds in Asia/Jakarta (WIB: UTC+7)
    let wib_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let now_wib = Utc::now().with_timezone(&wib_offset);
    let today_naive = now_wib.date_naive();
    let days_from_monday = today_naive.weekday().num_days_from_monday();
    let monday_naive = today_naive - Duration::days(days_from_monday as i64);
    let sunday_naive = monday_naive + Duration::days(6);

    let start_date_str = monday_naive.format("%Y-%m-%d").to_string();
    let end_date_str = sunday_naive.format("%Y-%m-%d").to_string();

    let rows = sqlx::query(
        r#"
        WITH student_acts AS (
            -- 1. Materials Completed
            SELECT 
                smc.id::text as id,
                lm.title as title,
                'MATERIAL' as category,
                'Materi Belajar' as category_label,
                COALESCE(lm.material_type, 'DOCUMENT') as sub_type,
                s.name as subject_name,
                smc.created_at AT TIME ZONE 'Asia/Jakarta' as act_time,
                to_char(smc.created_at AT TIME ZONE 'Asia/Jakarta', 'YYYY-MM-DD') as act_date_str
            FROM student_material_completions smc
            JOIN learning_materials lm ON lm.id = smc.material_id
            LEFT JOIN subjects s ON s.id = lm.subject_id
            WHERE smc.student_id = $1
              AND (smc.created_at AT TIME ZONE 'Asia/Jakarta')::date >= $2::date
              AND (smc.created_at AT TIME ZONE 'Asia/Jakarta')::date <= $3::date

            UNION ALL

            -- 2. Reading Progress
            SELECT 
                rp.id::text as id,
                lm.title as title,
                'MATERIAL' as category,
                'Materi Belajar' as category_label,
                COALESCE(lm.material_type, 'DOCUMENT') as sub_type,
                s.name as subject_name,
                COALESCE(rp.last_read_at, rp.created_at) AT TIME ZONE 'Asia/Jakarta' as act_time,
                to_char(COALESCE(rp.last_read_at, rp.created_at) AT TIME ZONE 'Asia/Jakarta', 'YYYY-MM-DD') as act_date_str
            FROM reading_progress rp
            JOIN learning_materials lm ON lm.id = rp.material_id
            LEFT JOIN subjects s ON s.id = lm.subject_id
            WHERE rp.student_id = $1
              AND (COALESCE(rp.last_read_at, rp.created_at) AT TIME ZONE 'Asia/Jakarta')::date >= $2::date
              AND (COALESCE(rp.last_read_at, rp.created_at) AT TIME ZONE 'Asia/Jakarta')::date <= $3::date
              AND NOT EXISTS (
                  SELECT 1 FROM student_material_completions c 
                  WHERE c.student_id = rp.student_id AND c.material_id = rp.material_id
                    AND (c.created_at AT TIME ZONE 'Asia/Jakarta')::date = (COALESCE(rp.last_read_at, rp.created_at) AT TIME ZONE 'Asia/Jakarta')::date
              )

            UNION ALL

            -- 3. Assignments Submitted
            SELECT 
                asb.id::text as id,
                a.title as title,
                'ASSIGNMENT' as category,
                'Tugas Siswa' as category_label,
                COALESCE(a.assignment_type, 'TUGAS') as sub_type,
                s.name as subject_name,
                COALESCE(asb.submitted_at, asb.created_at) AT TIME ZONE 'Asia/Jakarta' as act_time,
                to_char(COALESCE(asb.submitted_at, asb.created_at) AT TIME ZONE 'Asia/Jakarta', 'YYYY-MM-DD') as act_date_str
            FROM assignment_submissions asb
            JOIN assignments a ON a.id = asb.assignment_id
            LEFT JOIN subjects s ON s.id = a.subject_id
            WHERE asb.student_id = $1
              AND (COALESCE(asb.submitted_at, asb.created_at) AT TIME ZONE 'Asia/Jakarta')::date >= $2::date
              AND (COALESCE(asb.submitted_at, asb.created_at) AT TIME ZONE 'Asia/Jakarta')::date <= $3::date

            UNION ALL

            -- 4. Quizzes & CBT
            SELECT 
                qa.id::text as id,
                q.title as title,
                'QUIZ' as category,
                'Kuis & CBT' as category_label,
                COALESCE(q.exam_mode, 'QUIZ') as sub_type,
                s.name as subject_name,
                COALESCE(qa.completed_at, qa.started_at, qa.created_at) AT TIME ZONE 'Asia/Jakarta' as act_time,
                to_char(COALESCE(qa.completed_at, qa.started_at, qa.created_at) AT TIME ZONE 'Asia/Jakarta', 'YYYY-MM-DD') as act_date_str
            FROM quiz_attempts qa
            JOIN quizzes q ON q.id = qa.quiz_id
            LEFT JOIN subjects s ON s.id = q.subject_id
            WHERE qa.student_id = $1
              AND (COALESCE(qa.completed_at, qa.started_at, qa.created_at) AT TIME ZONE 'Asia/Jakarta')::date >= $2::date
              AND (COALESCE(qa.completed_at, qa.started_at, qa.created_at) AT TIME ZONE 'Asia/Jakarta')::date <= $3::date
        )
        SELECT id, title, category, category_label, sub_type, subject_name, to_char(act_time, 'HH24:MI') as act_hour, act_date_str
        FROM student_acts
        ORDER BY act_time DESC
        "#,
    )
    .bind(student_id)
    .bind(&start_date_str)
    .bind(&end_date_str)
    .fetch_all(&ctx.pool)
    .await?;

    let day_names = ["Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu"];
    let day_shorts = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];

    let mut days_stats: Vec<DailyActivityStat> = Vec::with_capacity(7);
    for i in 0..7 {
        let current_date = monday_naive + Duration::days(i as i64);
        let date_str = current_date.format("%Y-%m-%d").to_string();
        let is_today = current_date == today_naive;

        days_stats.push(DailyActivityStat {
            date: date_str,
            day_name: day_names[i].to_string(),
            day_short: day_shorts[i].to_string(),
            day_number: (i + 1) as i32,
            is_today,
            materials_count: 0,
            assignments_count: 0,
            quizzes_count: 0,
            total_count: 0,
            activities: Vec::new(),
        });
    }

    let mut total_materials = 0;
    let mut total_assignments = 0;
    let mut total_quizzes = 0;

    for row in rows {
        let id: String = row.get("id");
        let title: String = row.get("title");
        let category: String = row.get("category");
        let category_label: String = row.get("category_label");
        let sub_type: String = row.get("sub_type");
        let subject_name: Option<String> = row.get("subject_name");
        let act_hour: Option<String> = row.get("act_hour");
        let act_date_str: String = row.get("act_date_str");

        let time_str = act_hour.unwrap_or_else(|| "00:00".to_string());

        let item = DailyActivityItem {
            id,
            title,
            category: category.clone(),
            category_label,
            sub_type,
            subject_name,
            time: time_str,
        };

        if let Some(day_stat) = days_stats.iter_mut().find(|d| d.date == act_date_str) {
            match category.as_str() {
                "MATERIAL" => {
                    day_stat.materials_count += 1;
                    total_materials += 1;
                }
                "ASSIGNMENT" => {
                    day_stat.assignments_count += 1;
                    total_assignments += 1;
                }
                "QUIZ" => {
                    day_stat.quizzes_count += 1;
                    total_quizzes += 1;
                }
                _ => {}
            }
            day_stat.total_count += 1;
            day_stat.activities.push(item);
        }
    }

    let total_activities = total_materials + total_assignments + total_quizzes;

    let mut peak_day = "Senin".to_string();
    let mut peak_count = 0;
    let mut active_days_count = 0;

    for day in &days_stats {
        if day.total_count > peak_count {
            peak_count = day.total_count;
            peak_day = day.day_name.clone();
        }
        if day.total_count > 0 {
            active_days_count += 1;
        }
    }

    Ok(WeeklyActivityResponse {
        student_id,
        student_name,
        week_start_date: start_date_str,
        week_end_date: end_date_str,
        total_activities,
        total_materials,
        total_assignments,
        total_quizzes,
        peak_day,
        peak_count,
        active_days_count,
        days: days_stats,
    })
}

async fn get_my_study_tracker(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<StudyTrackerResponse>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthTokenExpired,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_row = sqlx::query(
        r#"
        SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1
        "#,
    )
    .bind(actor_id)
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

    let student_id = if let Some(s) = student_row {
        s.get::<Uuid, _>("id")
    } else {
        let child_row = sqlx::query(
            r#"
            SELECT s.id
            FROM guardians g
            JOIN students s ON s.guardian_id = g.id
            WHERE g.user_id = $1 AND g.tenant_id = $2
            LIMIT 1
            "#,
        )
        .bind(actor_id)
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

        if let Some(c) = child_row {
            c.get::<Uuid, _>("id")
        } else {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::ResourceNotFound,
                    "Data siswa tidak ditemukan untuk akun ini".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let resp = calculate_student_study_tracker(&ctx, req_ctx.tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_student_study_tracker_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
) -> Result<Json<ApiResponse<StudyTrackerResponse>>, ApiError> {
    let resp = calculate_student_study_tracker(&ctx, req_ctx.tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn calculate_student_study_tracker(
    ctx: &ApplicationContext,
    tenant_id: Uuid,
    student_id: Uuid,
) -> Result<StudyTrackerResponse, sqlx::Error> {
    // 1. Resolve student name & active class_id
    let student_info = sqlx::query(
        r#"
        SELECT 
            COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
            en.class_id
        FROM students s
        LEFT JOIN users u ON u.id = s.user_id
        LEFT JOIN enrollments en ON en.student_id = s.id AND (en.status = 'Active' OR en.status = 'ACTIVE')
        WHERE s.id = $1 AND s.tenant_id = $2
        LIMIT 1
        "#,
    )
    .bind(student_id)
    .bind(tenant_id)
    .fetch_optional(&ctx.pool)
    .await?;

    let (student_name, class_id) = match student_info {
        Some(row) => (
            row.get::<String, _>("student_name"),
            row.get::<Option<Uuid>, _>("class_id"),
        ),
        None => ("Siswa".to_string(), None),
    };

    // 2. Current Jakarta Date (WIB: UTC+7)
    let wib_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let now_wib = Utc::now().with_timezone(&wib_offset);
    let today_date_str = now_wib.date_naive().format("%Y-%m-%d").to_string();

    // ─────────────────────────────────────────────────────────────────────────────
    // 1. RIWAYAT BELAJAR (Sudah dibaca / dikerjakan)
    // ─────────────────────────────────────────────────────────────────────────────
    let mut history_items: Vec<TrackerItem> = Vec::new();

    // A. Completed Materials
    let mat_rows = sqlx::query(
        r#"
        SELECT 
            lm.id::text as id,
            lm.title,
            COALESCE(lm.material_type, 'DOCUMENT') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            to_char(smc.completed_at AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(smc.completed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM student_material_completions smc
        JOIN learning_materials lm ON lm.id = smc.material_id
        LEFT JOIN subjects s ON s.id = lm.subject_id
        WHERE smc.student_id = $1
        ORDER BY smc.completed_at DESC
        LIMIT 15
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in mat_rows {
        let id: String = r.get("id");
        history_items.push(TrackerItem {
            action_url: format!("/student/materials/{}", id),
            id,
            title: r.get("title"),
            item_type: "MATERIAL".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(100),
            score: None,
            max_score: None,
            timestamp_label: format!("Selesai • {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: "Tuntas 100%".to_string(),
            badge_variant: "success".to_string(),
            reason_note: Some("Materi pembelajaran ini telah kamu selesaikan.".to_string()),
        });
    }

    // B. Completed Quizzes
    let quiz_rows = sqlx::query(
        r#"
        SELECT 
            q.id::text as id,
            q.title,
            COALESCE(q.exam_mode, 'QUIZ') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            qa.score::float8 as score,
            q.max_score::float8 as max_score,
            to_char(COALESCE(qa.completed_at, qa.updated_at) AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(COALESCE(qa.completed_at, qa.updated_at), 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM quiz_attempts qa
        JOIN quizzes q ON q.id = qa.quiz_id
        LEFT JOIN subjects s ON s.id = q.subject_id
        WHERE qa.student_id = $1 AND qa.status = 'completed'
        ORDER BY qa.completed_at DESC
        LIMIT 15
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in quiz_rows {
        let id: String = r.get("id");
        let score: Option<f64> = r.get("score");
        let max_score: Option<f64> = r.get("max_score");
        let badge = match (score, max_score) {
            (Some(s), Some(m)) if m > 0.0 => format!("Nilai: {:.0}/{:.0}", s, m),
            (Some(s), _) => format!("Nilai: {:.0}", s),
            _ => "Selesai Dikerjakan".to_string(),
        };
        history_items.push(TrackerItem {
            action_url: format!("/student/quizzes/{}", id),
            id,
            title: r.get("title"),
            item_type: "QUIZ".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(100),
            score,
            max_score,
            timestamp_label: format!("Dikerjakan • {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: badge,
            badge_variant: "success".to_string(),
            reason_note: Some("Hasil evaluasi kuis telah tersimpan di rapor.".to_string()),
        });
    }

    // C. Submitted / Graded Assignments
    let assign_rows = sqlx::query(
        r#"
        SELECT 
            a.id::text as id,
            a.title,
            COALESCE(a.assignment_type, 'TUGAS') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            asb.score::float8 as score,
            a.max_score::float8 as max_score,
            asb.status as submission_status,
            to_char(COALESCE(asb.submitted_at, asb.created_at) AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(COALESCE(asb.submitted_at, asb.created_at), 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM assignment_submissions asb
        JOIN assignments a ON a.id = asb.assignment_id
        LEFT JOIN subjects s ON s.id = a.subject_id
        WHERE asb.student_id = $1 AND asb.status IN ('submitted', 'graded')
        ORDER BY asb.submitted_at DESC
        LIMIT 15
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in assign_rows {
        let id: String = r.get("id");
        let score: Option<f64> = r.get("score");
        let max_score: Option<f64> = r.get("max_score");
        let sub_status: String = r.get("submission_status");
        let badge = if sub_status == "graded" {
            match (score, max_score) {
                (Some(s), Some(m)) if m > 0.0 => format!("Dinilai: {:.0}/{:.0}", s, m),
                (Some(s), _) => format!("Dinilai: {:.0}", s),
                _ => "Dinilai Guru".to_string(),
            }
        } else {
            "Sudah Dikumpulkan".to_string()
        };
        history_items.push(TrackerItem {
            action_url: format!("/student/assignments/{}", id),
            id,
            title: r.get("title"),
            item_type: "ASSIGNMENT".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(100),
            score,
            max_score,
            timestamp_label: format!("Terkirim • {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: badge,
            badge_variant: "success".to_string(),
            reason_note: Some("Lembar tugas berhasil dikirimkan ke guru.".to_string()),
        });
    }

    // Sort history by date_iso descending
    history_items.sort_by(|a, b| b.date_iso.cmp(&a.date_iso));

    // ─────────────────────────────────────────────────────────────────────────────
    // 2. TERLEWAT (Materi/tugas sebelum hari ini yang belum dibuka/dikerjakan)
    // ─────────────────────────────────────────────────────────────────────────────
    let mut missed_items: Vec<TrackerItem> = Vec::new();

    // A. Unopened Materials published BEFORE today
    let unread_mat_rows = sqlx::query(
        r#"
        SELECT 
            lm.id::text as id,
            lm.title,
            COALESCE(lm.material_type, 'DOCUMENT') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            to_char(lm.created_at AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY') as date_str,
            to_char(lm.created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM learning_materials lm
        LEFT JOIN subjects s ON s.id = lm.subject_id
        WHERE lm.tenant_id = $1 
          AND lm.deleted_at IS NULL 
          AND lm.is_active = true
          AND (lm.class_id IS NULL OR lm.class_id = $2)
          AND (lm.created_at AT TIME ZONE 'Asia/Jakarta')::date < $3::date
          AND NOT EXISTS (
              SELECT 1 FROM student_material_completions smc 
              WHERE smc.material_id = lm.id AND smc.student_id = $4
          )
          AND NOT EXISTS (
              SELECT 1 FROM reading_progress rp 
              WHERE rp.material_id = lm.id AND rp.student_id = $4
          )
        ORDER BY lm.created_at DESC
        LIMIT 15
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .bind(&today_date_str)
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in unread_mat_rows {
        let id: String = r.get("id");
        missed_items.push(TrackerItem {
            action_url: format!("/student/materials/{}", id),
            id,
            title: r.get("title"),
            item_type: "MATERIAL".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(0),
            score: None,
            max_score: None,
            timestamp_label: format!("Terbit {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: "Belum Dibuka".to_string(),
            badge_variant: "warning".to_string(),
            reason_note: Some("Materi telah dibagikan sebelum hari ini tapi belum pernah kamu buka.".to_string()),
        });
    }

    // B. Overdue or unsubmitted assignments from before today
    let overdue_assign_rows = sqlx::query(
        r#"
        SELECT 
            a.id::text as id,
            a.title,
            COALESCE(a.assignment_type, 'TUGAS') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            to_char(COALESCE(a.due_at, a.created_at) AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(COALESCE(a.due_at, a.created_at), 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso,
            (a.due_at IS NOT NULL AND a.due_at < NOW()) as is_overdue
        FROM assignments a
        LEFT JOIN subjects s ON s.id = a.subject_id
        WHERE a.tenant_id = $1 
          AND a.deleted_at IS NULL 
          AND a.is_active = true
          AND (a.class_id IS NULL OR a.class_id = $2)
          AND ((a.created_at AT TIME ZONE 'Asia/Jakarta')::date < $3::date OR (a.due_at IS NOT NULL AND a.due_at < NOW()))
          AND NOT EXISTS (
              SELECT 1 FROM assignment_submissions asb 
              WHERE asb.assignment_id = a.id AND asb.student_id = $4 AND asb.status IN ('submitted', 'graded', 'returned')
          )
        ORDER BY a.due_at ASC NULLS LAST
        LIMIT 15
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .bind(&today_date_str)
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in overdue_assign_rows {
        let id: String = r.get("id");
        let is_overdue: bool = r.get("is_overdue");
        let badge = if is_overdue { "Lewat Tenggat" } else { "Belum Dikerjakan" };
        let variant = if is_overdue { "danger" } else { "warning" };
        missed_items.push(TrackerItem {
            action_url: format!("/student/assignments/{}", id),
            id,
            title: r.get("title"),
            item_type: "ASSIGNMENT".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(0),
            score: None,
            max_score: None,
            timestamp_label: format!("Tenggat: {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: badge.to_string(),
            badge_variant: variant.to_string(),
            reason_note: Some("Tugas diberikan sebelum hari ini dan belum dikumpulkan.".to_string()),
        });
    }

    // C. Missed Quizzes (published before today, not attempted)
    let missed_quiz_rows = sqlx::query(
        r#"
        SELECT 
            q.id::text as id,
            q.title,
            COALESCE(q.exam_mode, 'QUIZ') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            to_char(q.created_at AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY') as date_str,
            to_char(q.created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM quizzes q
        LEFT JOIN subjects s ON s.id = q.subject_id
        WHERE q.tenant_id = $1 
          AND q.deleted_at IS NULL 
          AND q.is_active = true
          AND (q.class_id IS NULL OR q.class_id = $2)
          AND (q.created_at AT TIME ZONE 'Asia/Jakarta')::date < $3::date
          AND NOT EXISTS (
              SELECT 1 FROM quiz_attempts qa 
              WHERE qa.quiz_id = q.id AND qa.student_id = $4
          )
        ORDER BY q.created_at DESC
        LIMIT 15
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .bind(&today_date_str)
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in missed_quiz_rows {
        let id: String = r.get("id");
        missed_items.push(TrackerItem {
            action_url: format!("/student/quizzes/{}", id),
            id,
            title: r.get("title"),
            item_type: "QUIZ".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(0),
            score: None,
            max_score: None,
            timestamp_label: format!("Jadwal: {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: "Kuis Terlewat".to_string(),
            badge_variant: "danger".to_string(),
            reason_note: Some("Kuis telah dirilis sebelum hari ini namun kamu belum mengikutinya.".to_string()),
        });
    }

    // D. Sessions missed due to absence / sick / excused before today
    let absent_session_rows = sqlx::query(
        r#"
        SELECT 
            ls.id::text as id,
            ls.title,
            'SESSION' as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            sa.status as attendance_status,
            to_char(ls.scheduled_at AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(ls.scheduled_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM learning_sessions ls
        JOIN session_attendances sa ON sa.session_id = ls.id
        LEFT JOIN subjects s ON s.id = ls.subject_id
        WHERE ls.tenant_id = $1 
          AND ls.deleted_at IS NULL
          AND sa.student_id = $4
          AND sa.status IN ('absent', 'sick', 'excused')
          AND (ls.scheduled_at AT TIME ZONE 'Asia/Jakarta')::date < $3::date
        ORDER BY ls.scheduled_at DESC
        LIMIT 10
        "#,
    )
    .bind(tenant_id)
    .bind(class_id)
    .bind(&today_date_str)
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in absent_session_rows {
        let id: String = r.get("id");
        let att_status: String = r.get("attendance_status");
        let (badge, note) = match att_status.as_str() {
            "sick" => ("Izin Sakit", "Kamu tercatat izin sakit pada sesi tatap muka ini. Cek materi kelas agar tidak tertinggal."),
            "excused" => ("Izin Tercatat", "Kamu tercatat izin pada sesi ini. Cek rangkuman materi dari guru."),
            _ => ("Tidak Hadir", "Kamu tidak hadir pada sesi tatap muka ini. Silakan pelajari materi mandiri."),
        };
        missed_items.push(TrackerItem {
            action_url: "/student/materials".to_string(),
            id,
            title: r.get("title"),
            item_type: "SESSION".to_string(),
            sub_type: Some("TATAP MUKA".to_string()),
            subject_name: r.get("subject_name"),
            progress_percent: Some(0),
            score: None,
            max_score: None,
            timestamp_label: format!("Sesi: {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: badge.to_string(),
            badge_variant: "warning".to_string(),
            reason_note: Some(note.to_string()),
        });
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // 3. BELUM SELESAI (Sebagian dibaca, revisi guru, aktivitas berjalan)
    // ─────────────────────────────────────────────────────────────────────────────
    let mut in_progress_items: Vec<TrackerItem> = Vec::new();

    // A. Partially read/watched materials
    let partial_reading_rows = sqlx::query(
        r#"
        SELECT 
            lm.id::text as id,
            lm.title,
            COALESCE(lm.material_type, 'DOCUMENT') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            COALESCE(rp.current_page, 1) as current_page,
            to_char(COALESCE(rp.last_read_at, rp.updated_at) AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(COALESCE(rp.last_read_at, rp.updated_at), 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM reading_progress rp
        JOIN learning_materials lm ON lm.id = rp.material_id
        LEFT JOIN subjects s ON s.id = lm.subject_id
        WHERE rp.student_id = $1 
          AND NOT rp.is_completed
          AND NOT EXISTS (
              SELECT 1 FROM student_material_completions smc 
              WHERE smc.material_id = lm.id AND smc.student_id = $1
          )
        ORDER BY rp.last_read_at DESC
        LIMIT 15
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in partial_reading_rows {
        let id: String = r.get("id");
        let page: i32 = r.get("current_page");
        let estimated_pct = ((page as f64 / 20.0) * 100.0).clamp(10.0, 90.0) as i32;
        in_progress_items.push(TrackerItem {
            action_url: format!("/student/materials/{}", id),
            id,
            title: r.get("title"),
            item_type: "MATERIAL".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(estimated_pct),
            score: None,
            max_score: None,
            timestamp_label: format!("Dibaca • Hal. {}", page),
            date_iso: Some(r.get("date_iso")),
            status_badge: format!("Dibaca {}%", estimated_pct),
            badge_variant: "info".to_string(),
            reason_note: Some("Materi baru kamu baca sebagian. Klik untuk melanjutkan membaca.".to_string()),
        });
    }

    // B. Assignments needing revision (returned by teacher)
    let returned_assign_rows = sqlx::query(
        r#"
        SELECT 
            a.id::text as id,
            a.title,
            COALESCE(a.assignment_type, 'TUGAS') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            asb.feedback,
            to_char(COALESCE(asb.updated_at, asb.submitted_at) AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(COALESCE(asb.updated_at, asb.submitted_at), 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM assignment_submissions asb
        JOIN assignments a ON a.id = asb.assignment_id
        LEFT JOIN subjects s ON s.id = a.subject_id
        WHERE asb.student_id = $1 
          AND asb.status = 'returned'
        ORDER BY asb.updated_at DESC
        LIMIT 10
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in returned_assign_rows {
        let id: String = r.get("id");
        let fb: Option<String> = r.get("feedback");
        let note = fb.unwrap_or_else(|| "Guru meminta tugas diperbaiki dan dikirimkan ulang.".to_string());
        in_progress_items.push(TrackerItem {
            action_url: format!("/student/assignments/{}", id),
            id,
            title: r.get("title"),
            item_type: "ASSIGNMENT".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(50),
            score: None,
            max_score: None,
            timestamp_label: format!("Dikembalikan • {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: "Perlu Perbaikan".to_string(),
            badge_variant: "danger".to_string(),
            reason_note: Some(format!("Catatan Guru: {}", note)),
        });
    }

    // C. Ongoing Quizzes (in_progress attempt)
    let ongoing_quiz_rows = sqlx::query(
        r#"
        SELECT 
            q.id::text as id,
            q.title,
            COALESCE(q.exam_mode, 'QUIZ') as sub_type,
            COALESCE(s.name, 'Mata Pelajaran') as subject_name,
            to_char(qa.started_at AT TIME ZONE 'Asia/Jakarta', 'DD Mon YYYY, HH24:MI') as date_str,
            to_char(qa.started_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as date_iso
        FROM quiz_attempts qa
        JOIN quizzes q ON q.id = qa.quiz_id
        LEFT JOIN subjects s ON s.id = q.subject_id
        WHERE qa.student_id = $1 
          AND qa.status = 'in_progress'
        ORDER BY qa.started_at DESC
        LIMIT 10
        "#,
    )
    .bind(student_id)
    .fetch_all(&ctx.pool)
    .await?;

    for r in ongoing_quiz_rows {
        let id: String = r.get("id");
        in_progress_items.push(TrackerItem {
            action_url: format!("/student/quizzes/{}/attempt", id),
            id,
            title: r.get("title"),
            item_type: "QUIZ".to_string(),
            sub_type: Some(r.get("sub_type")),
            subject_name: r.get("subject_name"),
            progress_percent: Some(50),
            score: None,
            max_score: None,
            timestamp_label: format!("Dimulai • {}", r.get::<String, _>("date_str")),
            date_iso: Some(r.get("date_iso")),
            status_badge: "Sedang Berlangsung".to_string(),
            badge_variant: "info".to_string(),
            reason_note: Some("Sesi kuis sedang berjalan. Segera selesaikan dan kumpulkan jawabanmu.".to_string()),
        });
    }

    let history_total = history_items.len() as i32;
    let missed_total = missed_items.len() as i32;
    let in_progress_total = in_progress_items.len() as i32;

    Ok(StudyTrackerResponse {
        student_id,
        student_name,
        history: TrackerCategory {
            key: "history".to_string(),
            title: "Riwayat Belajar".to_string(),
            description: "Materi yang sudah tuntas kamu pelajari & hasil kuis/tugasmu".to_string(),
            total_count: history_total,
            items: history_items,
        },
        missed: TrackerCategory {
            key: "missed".to_string(),
            title: "Terlewat".to_string(),
            description: "Materi, tugas, atau kuis sebelum hari ini yang belum sempat kamu buka/ikuti".to_string(),
            total_count: missed_total,
            items: missed_items,
        },
        in_progress: TrackerCategory {
            key: "in_progress".to_string(),
            title: "Belum Selesai".to_string(),
            description: "Materi yang baru dibaca sebagian, tugas revisi guru, & aktivitas berjalan".to_string(),
            total_count: in_progress_total,
            items: in_progress_items,
        },
    })
}
