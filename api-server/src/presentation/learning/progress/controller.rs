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
        .route("/student/{student_id}", get(get_student_progress_by_id))
        .route("/student/{student_id}/weekly-activity", get(get_student_weekly_activity_by_id))
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
