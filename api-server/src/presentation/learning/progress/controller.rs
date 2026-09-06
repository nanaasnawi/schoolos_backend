use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;
use chrono::Utc;

use super::dto::{
    calculate_progress_request::CalculateProgressRequest, progress_response::ProgressResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::progress::{
    calculate_progress::CalculateProgressCommand, get_progress::GetProgressQuery,
};

pub fn progress_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/calculate", post(calculate))
        .route("/me", get(get_my_progress))
        .route("/student/{student_id}", get(get_student_progress_by_id))
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

    let resp = calculate_dynamic_student_progress(&ctx, req_ctx.tenant_id, student_id, Some(class_id)).await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

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

    let resp = calculate_dynamic_student_progress(&ctx, req_ctx.tenant_id, student_id, class_id).await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn calculate_dynamic_student_progress(
    ctx: &ApplicationContext,
    tenant_id: Uuid,
    student_id: Uuid,
    class_id: Option<Uuid>,
) -> Result<ProgressResponse, sqlx::Error> {
    // 1. Learning materials
    let lesson_total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM learning_materials
        WHERE tenant_id = $1 AND deleted_at IS NULL AND is_active = true
          AND (class_id IS NULL OR class_id = $2)
        "#,
        tenant_id,
        class_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let lesson_completed = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM student_material_completions
        WHERE tenant_id = $1 AND student_id = $2
        "#,
        tenant_id,
        student_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 2. Assignments
    let assignment_total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM assignments
        WHERE tenant_id = $1 AND deleted_at IS NULL
          AND (class_id IS NULL OR class_id = $2)
        "#,
        tenant_id,
        class_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let assignment_completed = sqlx::query_scalar!(
        r#"
        SELECT COUNT(DISTINCT assignment_id)::int as "count!"
        FROM assignment_submissions
        WHERE student_id = $1 AND (status = 'submitted' OR status = 'graded')
        "#,
        student_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 3. Quizzes
    let quiz_total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM quizzes
        WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL
        "#,
        tenant_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let quiz_completed = sqlx::query_scalar!(
        r#"
        SELECT COUNT(DISTINCT quiz_id)::int as "count!"
        FROM quiz_attempts
        WHERE student_id = $1
        "#,
        student_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // 4. Sessions
    let session_total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM learning_sessions
        WHERE tenant_id = $1 AND (class_id IS NULL OR class_id = $2) AND status = 'completed' AND deleted_at IS NULL
        "#,
        tenant_id,
        class_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let session_attended = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::int as "count!"
        FROM session_attendances
        WHERE student_id = $1 AND (status = 'present' OR status = 'late')
        "#,
        student_id
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    // Calculate overall progress percentage
    let total_deliverables = lesson_total + assignment_total;
    let completed_deliverables = lesson_completed + assignment_completed;
    let overall_progress = if total_deliverables > 0 {
        (((completed_deliverables as f64) / (total_deliverables as f64)) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    // Query student name
    let student_name = sqlx::query_scalar!(
        r#"SELECT full_name FROM students WHERE id = $1 LIMIT 1"#,
        student_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Siswa".to_string());

    // Query class name and homeroom teacher name
    let (class_name, homeroom_teacher_name) = if let Some(cid) = class_id {
        let r = sqlx::query!(
            r#"
            SELECT c.name as class_name, t.full_name as "teacher_name?"
            FROM classes c
            LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id
            WHERE c.id = $1
            LIMIT 1
            "#,
            cid
        )
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        match r {
            Some(row) => (Some(row.class_name), row.teacher_name),
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
    } else if missing_assignments == 0 && total_deliverables > 0 && completed_deliverables == total_deliverables {
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

    let progress_res = ctx
        .get_progress
        .execute(query)
        .await;

    match progress_res {
        Ok(progress) => Ok(Json(ApiResponse::success(
            ProgressResponse::from(progress),
            req_ctx.request_id,
        ))),
        Err(_) => {
            // Fallback to dynamic calculation
            let dynamic = calculate_dynamic_student_progress(&ctx, req_ctx.tenant_id, student_id, Some(class_id)).await
                .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e)
                ), &req_ctx.request_id))?;
            Ok(Json(ApiResponse::success(dynamic, req_ctx.request_id)))
        }
    }
}
