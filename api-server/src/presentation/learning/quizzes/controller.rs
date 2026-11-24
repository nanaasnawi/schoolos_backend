use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    attempt_response::AttemptResponse,
    create_quiz_request::CreateQuizRequest,
    quiz_question_dto::{
        AttemptAnswerDetailDto, CreateQuizQuestionRequest,
        GradeAttemptRequest, QuizChoiceResponse, QuizQuestionResponse,
    },
    quiz_response::QuizResponse,
    start_attempt_request::StartAttemptRequest,
    submit_attempt_request::SubmitAttemptRequest,
    update_quiz_request::UpdateQuizRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::quiz::{
    create_quiz::CreateQuizCommand, get_quiz::GetQuizQuery, grade_attempt::GradeAttemptCommand,
    publish_quiz::PublishQuizCommand, start_attempt::StartAttemptCommand,
    submit_attempt::SubmitAnswer, submit_attempt::SubmitAttemptCommand,
};

static QUIZ_VARIATIF_SCHEMA_READY: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

pub(super) async fn ensure_quiz_variatif_schema(pool: &sqlx::PgPool) {
    QUIZ_VARIATIF_SCHEMA_READY
        .get_or_init(|| async {
            let statements = [
                "ALTER TABLE quiz_questions ALTER COLUMN question_type TYPE VARCHAR(40)",
                "ALTER TABLE quiz_questions ADD COLUMN IF NOT EXISTS interaction_config JSONB DEFAULT NULL",
                "ALTER TABLE attempt_answers ADD COLUMN IF NOT EXISTS teacher_feedback TEXT DEFAULT NULL",
                "ALTER TABLE quizzes ADD COLUMN IF NOT EXISTS notification_dispatched BOOLEAN DEFAULT FALSE",
            ];
            for sql in statements {
                if let Err(e) = sqlx::query(sql).execute(pool).await {
                    tracing::warn!("ensure_quiz_variatif_schema notice on `{sql}`: {e}");
                }
            }
        })
        .await;
}

/// Atomically claims notification dispatch once per quiz and sends targeted in-app + FCM
/// alerts only to active students enrolled in `target_class_id` (or all students if `None`).
async fn dispatch_quiz_published_notifications(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    quiz_id: Uuid,
    target_class_id: Option<Uuid>,
    quiz_title: &str,
    teacher_name: Option<&str>,
    class_name: Option<&str>,
) {
    let claimed = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE quizzes
        SET notification_dispatched = TRUE
        WHERE id = $1 AND tenant_id = $2 AND COALESCE(notification_dispatched, FALSE) = FALSE
        RETURNING id
        "#,
    )
    .bind(quiz_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if claimed.is_none() {
        return;
    }

    let notif_title = format!("🎯 Kuis & CBT Aktif: {}", quiz_title);
    let teacher_display = teacher_name.unwrap_or("Guru Pengampu");
    let notif_body = if let Some(c_name) = class_name {
        format!(
            "{} telah menerbitkan kuis/ujian CBT untuk kelas {}. Buka aplikasi untuk mengerjakan sebelum batas waktu!",
            teacher_display, c_name
        )
    } else {
        format!(
            "{} telah menerbitkan kuis/ujian CBT: {}. Buka aplikasi untuk memulai ujian!",
            teacher_display, quiz_title
        )
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
        SELECT
            gen_random_uuid(), s.tenant_id, s.user_id, $1, $2,
            'QUIZ', 'in_app', 'quiz', $4, FALSE, NOW()
        FROM students s
        JOIN enrollments en ON en.student_id = s.id
        WHERE s.tenant_id = $3
          AND ($5::uuid IS NULL OR en.class_id = $5)
          AND (en.status ILIKE 'active' OR en.status IS NULL)
          AND s.deleted_at IS NULL
        "#,
    )
    .bind(&notif_title)
    .bind(&notif_body)
    .bind(tenant_id)
    .bind(quiz_id)
    .bind(target_class_id)
    .execute(pool)
    .await;

    let fcm_target = if let Some(class_uuid) = target_class_id {
        crate::infrastructure::fcm::FcmTarget::Classes(vec![class_uuid])
    } else {
        crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()])
    };
    crate::infrastructure::fcm::trigger_fcm_push_targeted(
        notif_title,
        notif_body,
        crate::infrastructure::fcm::FcmCategory::Quiz,
        quiz_id,
        fcm_target,
    );
}

pub fn quiz_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route(
            "/proctoring/schedule",
            get(super::proctoring::get_proctor_schedule),
        )
        .route("/{id}", get(get_by_id).patch(update).put(update).delete(delete_quiz))
        .route("/{id}/publish", post(publish))
        .route("/{id}/verify-token", post(verify_token))
        .route(
            "/{id}/proctoring/monitor",
            get(super::proctoring::get_proctor_monitor),
        )
        .route(
            "/{id}/proctoring/rotate-token",
            post(super::proctoring::rotate_token),
        )
        .route(
            "/{id}/proctoring/extend-time",
            post(super::proctoring::extend_time),
        )
        .route("/{id}/questions", post(add_question).get(get_questions))
        .route("/{id}/attempts", post(start_attempt).get(get_attempts))
        .route("/{id}/attempts/{attempt_id}", get(get_attempt_by_id))
        .route(
            "/{id}/attempts/{attempt_id}/submit",
            post(submit_attempt),
        )
        .route(
            "/{id}/attempts/{attempt_id}/force-submit",
            post(super::proctoring::force_submit_attempt),
        )
        .route(
            "/{id}/attempts/{attempt_id}/reopen",
            post(super::proctoring::reopen_attempt),
        )
        .route(
            "/{id}/attempts/{attempt_id}/proctor-event",
            post(super::proctoring::record_proctor_event),
        )
        .route("/{id}/attempts/{attempt_id}/grade", post(grade_attempt))
        .route("/{id}/auto-grade", post(auto_grade_attempts))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateQuizRequest>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateQuizCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        title: payload.title,
        description: payload.description.clone(),
        duration_minutes: payload.duration_minutes.unwrap_or(30),
        passing_score: payload.passing_score,
        max_attempts: payload.max_attempts,
        shuffle_questions: payload.shuffle_questions,
        shuffle_choices: payload.shuffle_choices,
        start_at: payload.start_at,
        end_at: payload.end_at,
    };

    let quiz = ctx
        .create_quiz
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_strict_teacher = actor_is_strict_teacher(&req_ctx);
    let resolved_actor_teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM teachers WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 LIMIT 1"#,
        )
        .bind(aid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let validated_payload_teacher_id = if let Some(pid) = payload.teacher_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM teachers WHERE id = $1 AND tenant_id = $2 LIMIT 1"#,
        )
        .bind(pid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let teacher_id = if is_strict_teacher {
        resolved_actor_teacher_id.or(validated_payload_teacher_id)
    } else {
        validated_payload_teacher_id.or(resolved_actor_teacher_id)
    };

    // Resolve class_id from UUID or class name string (e.g. "PAKET C10")
    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar!(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                    req_ctx.tenant_id,
                    cid_str.trim()
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
        _ => {
            // Try resolving class from description second part: "Subject • Class • ..."
            if let Some(ref desc) = payload.description {
                let parts: Vec<&str> = desc.split('•').map(|s| s.trim()).collect();
                if parts.len() >= 2 && !parts[1].is_empty() {
                    let class_part = parts[1];
                    sqlx::query_scalar!(
                        r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2 OR name ILIKE '%' || $2 || '%') LIMIT 1"#,
                        req_ctx.tenant_id,
                        class_part
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
            }
        }
    };

    // Resolve subject_id from explicit payload or description first part
    let mut subject_id: Option<Uuid> = if let Some(sid) = payload.subject_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM subjects WHERE id = $1 AND tenant_id = $2 LIMIT 1"#,
        )
        .bind(sid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    if subject_id.is_none() {
        if let Some(ref desc) = payload.description {
            let first_part = desc.split('•').next().map(|s| s.trim()).unwrap_or("");
            if !first_part.is_empty() && !first_part.eq_ignore_ascii_case("Umum") {
                subject_id = sqlx::query_scalar!(
                    r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                    req_ctx.tenant_id,
                    first_part
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();
            }
        }
    }

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    ensure_quiz_variatif_schema(&ctx.pool).await;
    let exam_mode = payload.exam_mode.unwrap_or_else(|| "HOMEWORK_QUIZ".to_string());
    let assessment_category = crate::presentation::learning::grade_sync::normalize_assessment_category(
        payload.assessment_category.as_deref(),
        Some(&quiz.title),
    );
    let exam_token = payload.exam_token.filter(|t| !t.trim().is_empty());
    let requires_token = exam_token.as_deref().is_some_and(|t| !t.trim().is_empty());
    let max_token_attempts = payload.max_token_attempts.unwrap_or(5);
    let session_id = payload.session_id;

    let release_date = payload.release_at.or(payload.start_at);
    let is_scheduled = release_date.is_some_and(|r| r > chrono::Utc::now());
    let has_inline_questions = payload.questions.as_ref().is_some_and(|q| !q.is_empty());
    let defer_publish = payload.defer_publish.unwrap_or(false);
    let should_publish_immediately = !is_scheduled && !defer_publish && has_inline_questions;
    let initial_status = if should_publish_immediately {
        "published".to_string()
    } else {
        "draft".to_string()
    };

    let _ = sqlx::query(
        r#"UPDATE quizzes 
           SET class_id = $1, subject_id = $2, teacher_id = $3, created_by = $4,
               exam_mode = $5, exam_token = $6, max_token_attempts = $7, session_id = $8, token_expires_at = $9,
               status = $10, release_at = COALESCE($11, start_at, NOW()), notification_dispatched = FALSE,
               assessment_category = $13
           WHERE id = $12"#,
    )
    .bind(target_class_id)
    .bind(subject_id)
    .bind(teacher_id)
    .bind(actor_id)
    .bind(&exam_mode)
    .bind(&exam_token)
    .bind(max_token_attempts)
    .bind(session_id)
    .bind(payload.token_expires_at)
    .bind(&initial_status)
    .bind(release_date)
    .bind(quiz.id)
    .bind(&assessment_category)
    .execute(&ctx.pool)
    .await;

    let mut resp = QuizResponse::from(quiz);
    resp.status = initial_status;
    resp.release_at = release_date.or_else(|| Some(chrono::Utc::now()));
    resp.class_id = target_class_id;
    resp.session_id = session_id;
    resp.exam_mode = Some(exam_mode);
    resp.assessment_category = Some(assessment_category);
    resp.exam_token = exam_token;
    resp.requires_token = requires_token;
    resp.token_expires_at = payload.token_expires_at;
    resp.max_token_attempts = Some(max_token_attempts);
    if let Some(cid) = target_class_id {
        resp.class_name = sqlx::query_scalar!(r#"SELECT name FROM classes WHERE id = $1"#, cid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
    }
    if let Some(sid) = subject_id {
        resp.subject_name = sqlx::query_scalar!(r#"SELECT name FROM subjects WHERE id = $1"#, sid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();
    }
    resp.teacher_id = teacher_id;
    if let Some(tid) = teacher_id {
        resp.teacher_name =
            sqlx::query_scalar!(r#"SELECT full_name FROM teachers WHERE id = $1"#, tid)
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();
    }

    if let Some(questions) = payload.questions {
        ensure_quiz_variatif_schema(&ctx.pool).await;
        let q_count = questions.len() as i32;
        let mut total_pts = 0;
        for (idx, q) in questions.into_iter().enumerate() {
            let q_id = Uuid::new_v4();
            let q_type = q.question_type.unwrap_or_else(|| "MULTIPLE_CHOICE".to_string());
            let points = q.points.unwrap_or(10);
            total_pts += points;
            let order_idx = q.order_index.unwrap_or(idx as i32 + 1);
            let stored_q_text = merge_question_image_into_text(&q.question_text, q.image_url.as_deref());

            let _ = sqlx::query(
                r#"
                INSERT INTO quiz_questions (id, quiz_id, question_text, question_type, points, order_index, interaction_config, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
                "#,
            )
            .bind(q_id)
            .bind(resp.id)
            .bind(&stored_q_text)
            .bind(&q_type)
            .bind(points)
            .bind(order_idx)
            .bind(&q.interaction_config)
            .execute(&ctx.pool)
            .await;

            if let Some(choices) = q.choices {
                for (c_idx, c) in choices.into_iter().enumerate() {
                    let c_id = Uuid::new_v4();
                    let is_corr = c.is_correct.unwrap_or(false);
                    let c_order = c.order_index.unwrap_or(c_idx as i32 + 1);
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO quiz_choices (id, question_id, choice_text, is_correct, order_index, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
                        "#,
                    )
                    .bind(c_id)
                    .bind(q_id)
                    .bind(&c.choice_text)
                    .bind(is_corr)
                    .bind(c_order)
                    .execute(&ctx.pool)
                    .await;
                }
            }
        }

        let _ = sqlx::query(
            "UPDATE quizzes SET questions_count = $1, max_score = $2 WHERE id = $3",
        )
        .bind(q_count)
        .bind(total_pts)
        .bind(resp.id)
        .execute(&ctx.pool)
        .await;

        resp.questions_count = q_count;
        resp.max_score = total_pts;
    }

    resp.apply_schedule_evaluation(chrono::Utc::now());

    // Kirim notifikasi kuis baru ke siswa rombel target HANYA jika sudah diterbitkan bersama soal
    if should_publish_immediately {
        dispatch_quiz_published_notifications(
            &ctx.pool,
            req_ctx.tenant_id,
            resp.id,
            target_class_id,
            &resp.title,
            resp.teacher_name.as_deref(),
            resp.class_name.as_deref(),
        )
        .await;
    }

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<QuizResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let now_utc = chrono::Utc::now();
    let is_admin = actor_is_admin_or_principal(&req_ctx);

    let is_teacher = !is_admin && (
        req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n.contains("guru")
                        || n.contains("teacher")
                        || n.contains("pengajar")
                        || n.contains("tutor")
                        || n.contains("pamong")
                })
            })
            .unwrap_or(false)
            || crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(
                &ctx.pool,
                req_ctx.tenant_id,
                actor_id.unwrap_or_default(),
            )
            .await
            .ok()
            .flatten()
            .is_some()
    );
    let is_parent = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("wali")
                    || n.contains("parent")
                    || n.contains("guardian")
                    || n.contains("ortu")
            })
        })
        .unwrap_or(false);
    let is_student = !is_parent
        && !is_teacher
        && !is_admin
        && (req_ctx
            .actor
            .as_ref()
            .map(|a| {
                a.roles.iter().any(|r| {
                    let n = r.name.to_lowercase();
                    n == "siswa" || n == "student" || n == "murid" || n.contains("siswa")
                })
            })
            .unwrap_or(false)
            || crate::authorization_helpers::AuthorizationScope::resolve_student_id(
                &ctx.pool,
                req_ctx.tenant_id,
                actor_id.unwrap_or_default(),
            )
            .await
            .ok()
            .flatten()
            .is_some());

    let items: Vec<QuizResponse> = if is_teacher {
        let rows = sqlx::query(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, 
                CASE
                    WHEN q.end_at IS NOT NULL AND q.end_at <= NOW() AND (LOWER(COALESCE(q.status, '')) != 'draft' OR COALESCE(q.questions_count, 0) > 0) THEN 'closed'
                    WHEN COALESCE(q.release_at, q.start_at) > NOW() THEN 'draft'
                    ELSE q.status
                END as status, 
                q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id, q.teacher_id,
                q.session_id, q.exam_mode, q.assessment_category, q.exam_token, q.token_expires_at, q.max_token_attempts,
                q.release_at,
                c.name as class_name,
                sub.name as subject_name,
                t.full_name as teacher_name
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 
              AND q.deleted_at IS NULL
              AND (
                  q.created_by = $2 
                  OR q.teacher_id IN (SELECT id FROM teachers WHERE (user_id = $2 OR id = $2) AND tenant_id = $1)
                  OR q.teacher_id = $2
              )
            ORDER BY q.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.iter()
            .map(|r| build_quiz_response_from_list_row(r, false, false, now_utc))
            .collect()
    } else if is_student || is_parent {
        let rows = sqlx::query(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, q.status, q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id, q.teacher_id,
                q.session_id, q.exam_mode, q.assessment_category, q.exam_token, q.token_expires_at, q.max_token_attempts,
                q.release_at,
                c.name as class_name,
                sub.name as subject_name,
                t.full_name as teacher_name,
                (
                    SELECT qa.status FROM quiz_attempts qa
                    WHERE qa.quiz_id = q.id AND qa.tenant_id = q.tenant_id
                      AND (
                          qa.student_id = $2 
                          OR qa.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2)
                          OR qa.student_id IN (SELECT s.id FROM students s JOIN guardians g ON g.id = s.guardian_id WHERE g.user_id = $2)
                      )
                    ORDER BY qa.created_at DESC LIMIT 1
                ) AS student_attempt_status,
                (
                    SELECT COUNT(*)::int FROM quiz_attempts qa
                    WHERE qa.quiz_id = q.id AND qa.tenant_id = q.tenant_id
                      AND (
                          qa.student_id = $2 
                          OR qa.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2)
                          OR qa.student_id IN (SELECT s.id FROM students s JOIN guardians g ON g.id = s.guardian_id WHERE g.user_id = $2)
                      )
                ) AS student_attempts_count,
                (
                    SELECT qa.score FROM quiz_attempts qa
                    WHERE qa.quiz_id = q.id AND qa.tenant_id = q.tenant_id
                      AND (
                          qa.student_id = $2 
                          OR qa.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2)
                          OR qa.student_id IN (SELECT s.id FROM students s JOIN guardians g ON g.id = s.guardian_id WHERE g.user_id = $2)
                      )
                    ORDER BY qa.created_at DESC LIMIT 1
                ) AS student_last_score,
                (
                    SELECT qa.id FROM quiz_attempts qa
                    WHERE qa.quiz_id = q.id AND qa.tenant_id = q.tenant_id
                      AND (
                          qa.student_id = $2 
                          OR qa.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2)
                          OR qa.student_id IN (SELECT s.id FROM students s JOIN guardians g ON g.id = s.guardian_id WHERE g.user_id = $2)
                      )
                    ORDER BY qa.created_at DESC LIMIT 1
                ) AS student_last_attempt_id
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 
              AND q.deleted_at IS NULL
              AND (q.is_active = true OR LOWER(COALESCE(q.status, '')) = 'closed')
              AND (
                  q.status = 'published' 
                  OR q.status = 'active'
                  OR q.status = 'scheduled'
                  OR LOWER(COALESCE(q.status, '')) = 'closed'
                  OR (
                      q.status = 'draft'
                      AND COALESCE(q.questions_count, 0) > 0
                      AND COALESCE(q.release_at, q.start_at) IS NOT NULL
                      AND COALESCE(q.release_at, q.start_at) > NOW()
                  )
                  OR (COALESCE(q.release_at, q.start_at) IS NOT NULL AND COALESCE(q.release_at, q.start_at) <= NOW())
              )
              AND (
                  q.class_id IN (
                      SELECT en.class_id FROM enrollments en JOIN students s ON s.id = en.student_id WHERE (s.user_id = $2 OR s.id = $2) AND (en.status ILIKE 'active' OR en.status IS NULL)
                      UNION
                      SELECT en.class_id FROM enrollments en JOIN students s ON s.id = en.student_id JOIN guardians g ON g.id = s.guardian_id WHERE g.user_id = $2 AND (en.status ILIKE 'active' OR en.status IS NULL)
                  )
                  OR (
                      q.class_id IS NULL
                      AND NOT EXISTS (
                          SELECT 1 FROM classes other_c
                          WHERE other_c.tenant_id = q.tenant_id
                            AND (q.description ILIKE '%' || other_c.name || '%' OR q.title ILIKE '%' || other_c.name || '%')
                            AND other_c.id NOT IN (
                                SELECT en.class_id FROM enrollments en 
                                JOIN students s ON s.id = en.student_id 
                                WHERE s.user_id = $2 AND en.status ILIKE 'active'
                            )
                      )
                      AND (
                          CASE 
                              WHEN EXISTS (
                                  SELECT 1 FROM enrollments en
                                  JOIN students s ON s.id = en.student_id 
                                  JOIN classes sc ON sc.id = en.class_id
                                  WHERE s.user_id = $2 AND (sc.name ILIKE '%PAKET A%' OR sc.name ILIKE '%SD%')
                              ) THEN (
                                  q.title NOT ILIKE '%PAKET B%' AND q.title NOT ILIKE '%PAKET C%' 
                                  AND q.title NOT ILIKE '%SMP%' AND q.title NOT ILIKE '%SMA%' 
                                  AND (q.description IS NULL OR (
                                      q.description NOT ILIKE '%PAKET B%' AND q.description NOT ILIKE '%PAKET C%' 
                                      AND q.description NOT ILIKE '%SMP%' AND q.description NOT ILIKE '%SMA%'
                                  ))
                              )
                              ELSE true
                          END
                      )
                  )
              )
            ORDER BY q.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.iter()
            .map(|r| build_quiz_response_from_list_row(r, true, true, now_utc))
            .collect()
    } else {
        let rows = sqlx::query(
            r#"
            SELECT 
                q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
                q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
                q.start_at, q.end_at, 
                CASE
                    WHEN q.end_at IS NOT NULL AND q.end_at <= NOW() AND (LOWER(COALESCE(q.status, '')) != 'draft' OR COALESCE(q.questions_count, 0) > 0) THEN 'closed'
                    WHEN COALESCE(q.release_at, q.start_at) > NOW() THEN 'draft'
                    ELSE q.status
                END as status, 
                q.questions_count, q.is_active,
                q.created_at, q.updated_at,
                q.class_id, q.teacher_id,
                q.session_id, q.exam_mode, q.assessment_category, q.exam_token, q.token_expires_at, q.max_token_attempts,
                q.release_at,
                c.name as class_name,
                sub.name as subject_name,
                t.full_name as teacher_name
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN subjects sub ON sub.id = q.subject_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.tenant_id = $1 AND q.deleted_at IS NULL
            ORDER BY q.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        rows.iter()
            .map(|r| build_quiz_response_from_list_row(r, false, false, now_utc))
            .collect()
    };

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

pub(super) async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let meta = sqlx::query(
        r#"
        SELECT q.tenant_id, q.class_id, q.teacher_id, q.created_by, q.session_id,
               q.exam_mode, q.assessment_category, q.exam_token, q.token_expires_at,
               q.max_token_attempts, q.release_at, q.status, q.start_at, q.is_active, q.questions_count,
               c.name AS class_name,
               sub.name AS subject_name,
               COALESCE(NULLIF(TRIM(t.full_name), ''), uc.full_name) AS teacher_name
        FROM quizzes q
        LEFT JOIN classes c ON c.id = q.class_id
        LEFT JOIN subjects sub ON sub.id = q.subject_id
        LEFT JOIN teachers t ON t.id = q.teacher_id
        LEFT JOIN users uc ON uc.id = q.created_by
        WHERE q.id = $1 AND q.tenant_id = $2 AND q.deleted_at IS NULL
        "#,
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta_tenant_id: Uuid = meta.get("tenant_id");
    let meta_class_id: Option<Uuid> = meta.get("class_id");
    let meta_teacher_id: Option<Uuid> = meta.get("teacher_id");
    let meta_created_by: Option<Uuid> = meta.get("created_by");

    // Strict Cross-Class and Multi-Tenant Access Verification
    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta_tenant_id,
        meta_class_id,
        meta_teacher_id,
        meta_created_by,
    )
    .await?;

    let is_student_or_parent = !actor_is_teacher_or_admin(&req_ctx);
    let meta_release_at: Option<chrono::DateTime<chrono::Utc>> = meta.try_get("release_at").ok().flatten();
    let meta_start_at: Option<chrono::DateTime<chrono::Utc>> = meta.try_get("start_at").ok().flatten();
    let meta_status: String = meta.try_get("status").unwrap_or_else(|_| "draft".to_string());
    let meta_is_active: bool = meta.try_get("is_active").unwrap_or(true);
    let meta_questions_count: i32 = meta.try_get("questions_count").unwrap_or(0);
    if is_student_or_parent {
        let now_utc = chrono::Utc::now();
        let is_open_status = meta_status.eq_ignore_ascii_case("published")
            || meta_status.eq_ignore_ascii_case("active")
            || meta_status.eq_ignore_ascii_case("live_exam")
            || meta_status.eq_ignore_ascii_case("live");
        let is_closed_status = meta_status.eq_ignore_ascii_case("closed");
        let is_scheduled_visible = meta_status.eq_ignore_ascii_case("scheduled")
            || (meta_status.eq_ignore_ascii_case("draft")
                && meta_questions_count > 0
                && meta_release_at.or(meta_start_at).is_some_and(|r| r > now_utc));
        if (!meta_is_active && !is_closed_status) || (!is_open_status && !is_scheduled_visible && !is_closed_status) {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::QuizNotFound,
                    "Ujian CBT belum diterbitkan untuk siswa.".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let query = GetQuizQuery { quiz_id: id };
    let quiz = ctx
        .get_quiz
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let raw_exam_token: Option<String> = meta.try_get("exam_token").ok().flatten();
    let requires_token = raw_exam_token
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty());

    let mut resp = QuizResponse::from(quiz);
    resp.session_id = meta.get("session_id");
    resp.release_at = meta_release_at;
    resp.exam_mode = meta.get::<Option<String>, _>("exam_mode").or_else(|| Some("HOMEWORK_QUIZ".to_string()));
    resp.assessment_category = meta.try_get::<Option<String>, _>("assessment_category").ok().flatten().or_else(|| Some("FORMATIVE".to_string()));
    resp.exam_token = if is_student_or_parent {
        None
    } else {
        raw_exam_token
    };
    resp.requires_token = requires_token;
    resp.token_expires_at = meta.get("token_expires_at");
    resp.max_token_attempts = meta.get::<Option<i32>, _>("max_token_attempts").or(Some(5));
    resp.class_id = meta_class_id;
    resp.class_name = meta.try_get("class_name").ok().flatten();
    resp.subject_name = meta.try_get("subject_name").ok().flatten();
    resp.teacher_id = meta_teacher_id;
    resp.teacher_name = meta.try_get("teacher_name").ok().flatten();
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    let attempt_info = sqlx::query(
        r#"
        SELECT 
            qa.status,
            (SELECT COUNT(*)::int FROM quiz_attempts qa2 WHERE qa2.quiz_id = $1 AND (qa2.student_id = $2 OR qa2.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2))) as attempts_count,
            qa.score,
            qa.id as attempt_id
        FROM quiz_attempts qa
        WHERE qa.quiz_id = $1 AND qa.tenant_id = $3
          AND (qa.student_id = $2 OR qa.student_id IN (SELECT s.id FROM students s WHERE s.user_id = $2))
        ORDER BY qa.created_at DESC LIMIT 1
        "#
    )
    .bind(id)
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    if let Some(r) = attempt_info {
        let status: Option<String> = r.get("status");
        let count: Option<i32> = r.get("attempts_count");
        let score = mask_ungraded_score(
            r.get("score"),
            status.as_deref(),
            !is_student_or_parent,
        );
        let att_id: Option<Uuid> = r.get("attempt_id");
        let count_val = count.unwrap_or(0);
        let is_completed = status.as_deref().map(|s| {
            let s_lower = s.to_lowercase();
            s_lower == "completed" || s_lower == "submitted" || s_lower == "graded"
        }).unwrap_or(false) || (count_val > 0 && count_val >= resp.max_attempts);

        resp.student_attempt_status = status;
        resp.student_attempts_count = count;
        resp.student_has_completed = Some(is_completed);
        resp.student_last_score = score;
        resp.student_last_attempt_id = att_id;
    }

    resp.apply_schedule_evaluation(chrono::Utc::now());

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateQuizRequest>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta_tenant_id: Uuid = meta.get("tenant_id");
    let meta_class_id: Option<Uuid> = meta.get("class_id");
    let meta_teacher_id: Option<Uuid> = meta.get("teacher_id");
    let meta_created_by: Option<Uuid> = meta.get("created_by");

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta_tenant_id,
        meta_class_id,
        meta_teacher_id,
        meta_created_by,
    )
    .await?;

    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                )
                .bind(req_ctx.tenant_id)
                .bind(cid_str.trim())
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
        _ => None,
    };

    let is_published_status = payload
        .status
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("Published"))
        .unwrap_or(false);

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    let normalized_cat = payload.assessment_category.as_deref().map(|c| {
        crate::presentation::learning::grade_sync::normalize_assessment_category(
            Some(c),
            payload.title.as_deref(),
        )
    });

    let validated_teacher_id = if let Some(tid) = payload.teacher_id {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM teachers WHERE id = $1 AND tenant_id = $2 LIMIT 1",
        )
        .bind(tid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let validated_subject_id = if let Some(sid) = payload.subject_id {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM subjects WHERE id = $1 AND tenant_id = $2 LIMIT 1",
        )
        .bind(sid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let has_start_update = payload.start_at.is_some();
    let has_end_update = payload.end_at.is_some();

    let _ = sqlx::query(
        r#"
        UPDATE quizzes
        SET title = COALESCE($1, title),
            description = COALESCE($2, description),
            time_limit_minutes = COALESCE($3, time_limit_minutes),
            passing_score = COALESCE($4, passing_score),
            max_attempts = COALESCE($5, max_attempts),
            shuffle_questions = COALESCE($6, shuffle_questions),
            shuffle_choices = COALESCE($7, shuffle_choices),
            start_at = CASE WHEN $8 THEN $9 ELSE start_at END,
            release_at = CASE WHEN $8 THEN $9 ELSE release_at END,
            end_at = CASE WHEN $10 THEN $11 ELSE end_at END,
            class_id = COALESCE($12, class_id),
            session_id = COALESCE($13, session_id),
            exam_mode = COALESCE($14, exam_mode),
            exam_token = COALESCE($15, exam_token),
            token_expires_at = COALESCE($16, token_expires_at),
            max_token_attempts = COALESCE($17, max_token_attempts),
            status = CASE
                WHEN $18 IS NOT NULL THEN $18
                WHEN $10 AND $11 > NOW() AND LOWER(COALESCE(status, '')) = 'closed' THEN 'published'
                ELSE status
            END,
            is_active = CASE
                WHEN $10 AND $11 > NOW() AND LOWER(COALESCE($18, status, '')) NOT IN ('closed', 'archived') THEN true
                ELSE is_active
            END,
            assessment_category = COALESCE($21, assessment_category),
            teacher_id = COALESCE($22, teacher_id),
            subject_id = COALESCE($23, subject_id),
            updated_at = NOW()
        WHERE id = $19 AND tenant_id = $20 AND deleted_at IS NULL
        "#
    )
    .bind(payload.title)
    .bind(payload.description)
    .bind(payload.duration_minutes)
    .bind(payload.passing_score)
    .bind(payload.max_attempts)
    .bind(payload.shuffle_questions)
    .bind(payload.shuffle_choices)
    .bind(has_start_update)
    .bind(payload.start_at)
    .bind(has_end_update)
    .bind(payload.end_at)
    .bind(target_class_id)
    .bind(payload.session_id)
    .bind(payload.exam_mode)
    .bind(payload.exam_token)
    .bind(payload.token_expires_at)
    .bind(payload.max_token_attempts)
    .bind(payload.status)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .bind(normalized_cat)
    .bind(validated_teacher_id)
    .bind(validated_subject_id)
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

    if has_start_update {
        crate::infrastructure::scheduled_publish_worker::clear_quiz_pre_start_notice(&ctx.pool, id)
            .await;
    }

    if is_published_status {
        let meta = sqlx::query(
            r#"
            SELECT q.class_id, q.title, c.name AS class_name, t.full_name AS teacher_name
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.id = $1 AND q.tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();
        if let Some(r) = meta {
            let q_class: Option<uuid::Uuid> = r.try_get("class_id").ok();
            let q_title: String = r.try_get("title").unwrap_or_else(|_| "Kuis".to_string());
            let q_class_name: Option<String> = r.try_get("class_name").ok().flatten();
            let q_teacher_name: Option<String> = r.try_get("teacher_name").ok().flatten();
            dispatch_quiz_published_notifications(
                &ctx.pool,
                req_ctx.tenant_id,
                id,
                q_class,
                &q_title,
                q_teacher_name.as_deref(),
                q_class_name.as_deref(),
            )
            .await;
        }
    }

    get_by_id(State(ctx), req_ctx, Path(id)).await
}

async fn delete_quiz(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta_tenant_id: Uuid = meta.get("tenant_id");
    let meta_class_id: Option<Uuid> = meta.get("class_id");
    let meta_teacher_id: Option<Uuid> = meta.get("teacher_id");
    let meta_created_by: Option<Uuid> = meta.get("created_by");

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta_tenant_id,
        meta_class_id,
        meta_teacher_id,
        meta_created_by,
    )
    .await?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    sqlx::query(
        "UPDATE quizzes SET deleted_at = NOW(), deleted_by = $1, is_active = false, updated_at = NOW() WHERE id = $2 AND tenant_id = $3",
    )
    .bind(actor_id)
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

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishQuizCommand { quiz_id: id };
    let quiz = ctx
        .publish_quiz
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // ── FCM + in-app untuk KUIS/CBT yang di-publish (hanya sekali via atomic guard) ──
    {
        let meta = sqlx::query(
            r#"
            SELECT q.class_id, c.name AS class_name, t.full_name AS teacher_name
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            WHERE q.id = $1 AND q.tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();
        let (q_class, q_class_name, q_teacher_name): (Option<Uuid>, Option<String>, Option<String>) =
            match meta {
                Some(r) => (
                    r.try_get("class_id").ok(),
                    r.try_get("class_name").ok().flatten(),
                    r.try_get("teacher_name").ok().flatten(),
                ),
                None => (None, None, None),
            };
        dispatch_quiz_published_notifications(
            &ctx.pool,
            req_ctx.tenant_id,
            id,
            q_class,
            &quiz.title,
            q_teacher_name.as_deref(),
            q_class_name.as_deref(),
        )
        .await;
    }

    get_by_id(State(ctx), req_ctx, Path(id)).await
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct VerifyQuizTokenRequest {
    pub token: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct VerifyQuizTokenResponse {
    pub valid: bool,
    pub message: String,
}

async fn verify_token(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<VerifyQuizTokenRequest>,
) -> Result<Json<ApiResponse<VerifyQuizTokenResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, status, start_at, end_at, release_at, exam_token, token_expires_at FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let meta_status: String = quiz_meta.try_get("status").unwrap_or_else(|_| "draft".to_string());
    let meta_release_at: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("release_at").ok().flatten();
    let meta_start_at: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("start_at").ok().flatten();
    let meta_end_at: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("end_at").ok().flatten();
    let token_expires_at: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("token_expires_at").ok().flatten();
    let exam_token_db: Option<String> = quiz_meta.try_get("exam_token").ok().flatten();

    let now_utc = chrono::Utc::now();
    if !meta_status.eq_ignore_ascii_case("published") && !meta_status.eq_ignore_ascii_case("active") {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Ujian CBT belum diterbitkan.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }
    if meta_release_at.or(meta_start_at).is_some_and(|s| now_utc < s) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Ujian CBT belum memasuki jadwal mulai.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }
    if meta_end_at.is_some_and(|e| now_utc > e) || token_expires_at.is_some_and(|exp| now_utc > exp) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Batas waktu atau masa berlaku token ujian telah berakhir.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let student_id = crate::authorization_helpers::AuthorizationScope::resolve_student_id(
        &ctx.pool,
        req_ctx.tenant_id,
        actor_id,
    )
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .unwrap_or(actor_id);

    // 1. Cek status percobaan token pada quiz_token_attempts
    let attempt_record = sqlx::query(
        r#"
        SELECT failed_attempts, locked_until
        FROM quiz_token_attempts
        WHERE tenant_id = $1 AND quiz_id = $2 AND student_id = $3
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(id)
    .bind(student_id)
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

    let current_fails_count: i32 = if let Some(ref rec) = attempt_record {
        let locked_until: Option<chrono::DateTime<chrono::Utc>> = rec.try_get("locked_until").ok().flatten();
        if let Some(locked_until) = locked_until {
            if locked_until > now_utc {
                let remaining_secs = (locked_until - now_utc).num_seconds();
                let remaining_mins = (remaining_secs / 60) + 1;
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            format!(
                                "Token ujian terkunci selama {} menit karena 5 kali salah input berturut-turut.",
                                remaining_mins
                            ),
                        ),
                    ),
                    &req_ctx.request_id,
                ));
            }
        }
        rec.try_get("failed_attempts").unwrap_or(0)
    } else {
        0
    };

    let input_token = payload.token.trim().to_uppercase();
    let expects_token = exam_token_db
        .as_deref()
        .is_some_and(|expected| !expected.trim().is_empty());
    if expects_token && input_token.is_empty() {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Token ujian wajib diisi terlebih dahulu.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let is_valid = match exam_token_db {
        Some(ref expected) if !expected.trim().is_empty() => {
            expected.trim().to_uppercase() == input_token
        }
        _ => true,
    };

    if is_valid {
        let _ = sqlx::query(
            r#"
            INSERT INTO quiz_token_attempts (id, tenant_id, quiz_id, student_id, failed_attempts, locked_until, last_attempt_at)
            VALUES (gen_random_uuid(), $1, $2, $3, 0, NULL, NOW())
            ON CONFLICT (tenant_id, quiz_id, student_id)
            DO UPDATE SET failed_attempts = 0, locked_until = NULL, last_attempt_at = NOW()
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(id)
        .bind(student_id)
        .execute(&ctx.pool)
        .await;

        Ok(Json(ApiResponse::success(
            VerifyQuizTokenResponse {
                valid: true,
                message: "Token ujian valid. Selamat mengerjakan!".to_string(),
            },
            req_ctx.request_id,
        )))
    } else {
        let current_fails = current_fails_count + 1;
        let lock_time = if current_fails >= 5 {
            Some(now_utc + chrono::Duration::minutes(15))
        } else {
            None
        };

        let _ = sqlx::query(
            r#"
            INSERT INTO quiz_token_attempts (id, tenant_id, quiz_id, student_id, failed_attempts, locked_until, last_attempt_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())
            ON CONFLICT (tenant_id, quiz_id, student_id)
            DO UPDATE SET failed_attempts = $4, locked_until = $5, last_attempt_at = NOW()
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(id)
        .bind(student_id)
        .bind(current_fails)
        .bind(lock_time)
        .execute(&ctx.pool)
        .await;

        let rem = 5 - current_fails;
        let msg = if rem <= 0 {
            "Token ujian salah. Akun Anda terkunci dari memasukkan token selama 15 menit.".to_string()
        } else {
            format!("Token ujian salah. Sisa kesempatan mencoba: {} kali.", rem)
        };

        Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(msg),
            ),
            &req_ctx.request_id,
        ))
    }
}

async fn start_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<StartAttemptRequest>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, exam_token, token_expires_at, release_at, status, start_at, end_at, is_active FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let meta_tenant_id: Uuid = meta.get("tenant_id");
    let meta_class_id: Option<Uuid> = meta.get("class_id");
    let meta_teacher_id: Option<Uuid> = meta.get("teacher_id");
    let meta_created_by: Option<Uuid> = meta.get("created_by");
    let meta_exam_token: Option<String> = meta.try_get("exam_token").ok().flatten();
    let meta_token_expires_at: Option<chrono::DateTime<chrono::Utc>> =
        meta.try_get("token_expires_at").ok().flatten();
    let meta_release_at: Option<chrono::DateTime<chrono::Utc>> = meta.try_get("release_at").ok().flatten();
    let meta_start_at: Option<chrono::DateTime<chrono::Utc>> = meta.try_get("start_at").ok().flatten();
    let meta_end_at: Option<chrono::DateTime<chrono::Utc>> = meta.try_get("end_at").ok().flatten();
    let meta_status: String = meta.try_get("status").unwrap_or_else(|_| "draft".to_string());
    let meta_is_active: bool = meta.try_get("is_active").unwrap_or(true);

    // Verify student belongs to the class of this quiz
    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta_tenant_id,
        meta_class_id,
        meta_teacher_id,
        meta_created_by,
    )
    .await?;

    if !meta_is_active
        || meta_status.eq_ignore_ascii_case("closed")
        || meta_status.eq_ignore_ascii_case("archived")
        || meta_end_at.is_some_and(|e| chrono::Utc::now() >= e)
    {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Waktu pelaksanaan ujian CBT telah berakhir.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    if !meta_status.eq_ignore_ascii_case("published")
        && !meta_status.eq_ignore_ascii_case("active")
        && !meta_status.eq_ignore_ascii_case("live_exam")
        && !meta_status.eq_ignore_ascii_case("live")
    {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Ujian CBT belum diterbitkan oleh guru.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }
    if meta_release_at.or(meta_start_at).is_some_and(|r| chrono::Utc::now() < r) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Ujian CBT belum memasuki waktu mulai.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let effective_student_id =
        crate::authorization_helpers::AuthorizationScope::resolve_student_id(
            &ctx.pool,
            req_ctx.tenant_id,
            actor_id,
        )
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?
        .unwrap_or_else(|| payload.student_id.unwrap_or(actor_id));

    // Cek apakah siswa terkunci karena token rate limit
    let is_locked = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM quiz_token_attempts WHERE tenant_id = $1 AND quiz_id = $2 AND student_id = $3 AND locked_until > NOW())"
    )
    .bind(req_ctx.tenant_id)
    .bind(id)
    .bind(effective_student_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(false);

    if is_locked {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Siswa terkunci dari ujian ini karena 5 kali gagal memasukkan token. Silakan tunggu 15 menit.".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    // Jika kuis memiliki exam_token aktif, wajibkan verifikasi token terlebih dahulu
    if let Some(ref expected_token) = meta_exam_token {
        let clean_expected = expected_token.trim();
        if !clean_expected.is_empty() {
            if let Some(exp) = meta_token_expires_at {
                if exp < chrono::Utc::now() {
                    return Err(ApiError::new(
                        school_core::common::error::ApplicationError::Domain(
                            school_core::common::error::DomainError::Validation(
                                "Token ujian telah kedaluwarsa. Hubungi pengawas atau guru untuk memperbarui token ujian.".to_string(),
                            ),
                        ),
                        &req_ctx.request_id,
                    ));
                }
            }

            let provided_matches = payload
                .exam_token
                .as_deref()
                .map(|t| t.trim().eq_ignore_ascii_case(clean_expected))
                .unwrap_or(false);

            let previously_verified = if provided_matches {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO quiz_token_attempts (id, tenant_id, quiz_id, student_id, failed_attempts, locked_until, last_attempt_at)
                    VALUES (gen_random_uuid(), $1, $2, $3, 0, NULL, NOW())
                    ON CONFLICT (tenant_id, quiz_id, student_id)
                    DO UPDATE SET failed_attempts = 0, locked_until = NULL, last_attempt_at = NOW()
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(id)
                .bind(effective_student_id)
                .execute(&ctx.pool)
                .await;
                true
            } else {
                sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM quiz_token_attempts
                        WHERE tenant_id = $1 AND quiz_id = $2 AND student_id = $3
                          AND failed_attempts = 0 AND last_attempt_at IS NOT NULL
                          AND (locked_until IS NULL OR locked_until <= NOW())
                    )
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(id)
                .bind(effective_student_id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap_or(false)
            };

            if !previously_verified {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            "Token ujian wajib diverifikasi terlebih dahulu sebelum memulai pengerjaan kuis/CBT.".to_string(),
                        ),
                    ),
                    &req_ctx.request_id,
                ));
            }
        }
    }

    // Sinkronkan max_score & questions_count sebelum start agar total_points akurat
    let _ = sqlx::query(
        r#"
        UPDATE quizzes
        SET questions_count = COALESCE((SELECT COUNT(*)::int FROM quiz_questions WHERE quiz_id = $1), 0),
            max_score = COALESCE((SELECT SUM(points)::int FROM quiz_questions WHERE quiz_id = $1), 0)
        WHERE id = $1 AND tenant_id = $2 AND (max_score <= 0 OR questions_count <= 0)
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await;

    let command = StartAttemptCommand {
        tenant_id: req_ctx.tenant_id,
        quiz_id: id,
        student_id: effective_student_id,
    };
    let attempt = ctx
        .start_attempt
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttemptResponse::from(attempt),
        req_ctx.request_id,
    )))
}

async fn submit_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, attempt_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SubmitAttemptRequest>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_teacher_actor = actor_is_teacher_or_admin(&req_ctx);
    let is_student = !is_teacher_actor;

    if is_student {
        let student_id = crate::authorization_helpers::AuthorizationScope::resolve_student_id(
            &ctx.pool,
            req_ctx.tenant_id,
            actor_id,
        )
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?
        .unwrap_or(actor_id);

        let attempt_owner =
            sqlx::query("SELECT student_id FROM quiz_attempts WHERE id = $1 AND quiz_id = $2 AND tenant_id = $3")
                .bind(attempt_id)
                .bind(_id)
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
                            school_core::common::error_code::ErrorCode::AttemptNotFound,
                            format!("Attempt {} not found", attempt_id),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

        let attempt_owner_id: Uuid = attempt_owner.get("student_id");
        if attempt_owner_id != student_id {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Anda tidak berhak mengumpulkan jawaban untuk pengerjaan kuis ini".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let answers = payload
        .answers
        .into_iter()
        .map(|a| SubmitAnswer {
            question_id: a.question_id,
            chosen_choice_id: a.chosen_choice_id,
            text_answer: a.text_answer,
        })
        .collect();

    let command = SubmitAttemptCommand {
        attempt_id,
        answers,
    };
    let attempt = ctx
        .submit_attempt
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    ensure_quiz_variatif_schema(&ctx.pool).await;
    let attempt_is_graded = attempt.status.eq_ignore_ascii_case("graded");

    let answer_rows = sqlx::query(
        r#"
        SELECT 
            ans.question_id,
            q.question_text,
            q.question_type,
            q.points as max_points,
            q.interaction_config,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            ans.is_correct as ans_is_correct,
            c.is_correct as choice_is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback
        FROM attempt_answers ans
        JOIN quiz_attempts qa ON qa.id = ans.attempt_id
        JOIN quiz_questions q ON q.id = ans.question_id
        LEFT JOIN quiz_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.attempt_id = $1 AND qa.tenant_id = $2
        ORDER BY q.order_index ASC, q.created_at ASC
        "#,
    )
    .bind(attempt_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let answers: Vec<AttemptAnswerDetailDto> = answer_rows
        .iter()
        .map(|a| build_attempt_answer_detail_dto(a, is_teacher_actor, attempt_is_graded))
        .collect();

    let mut resp = AttemptResponse::from(attempt);
    resp.answers = answers;
    if !is_teacher_actor && !attempt_is_graded {
        resp.score = None;
        resp.percentage = None;
        resp.passed = None;
    }

    if resp.status.eq_ignore_ascii_case("graded") && resp.score.is_some() {
        crate::presentation::learning::grade_sync::sync_quiz_attempt_to_gradebook(
            &ctx.pool,
            req_ctx.tenant_id,
            resp.id,
        )
        .await;
    }

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn grade_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((quiz_id, attempt_id)): Path<(Uuid, Uuid)>,
    payload: Option<Json<GradeAttemptRequest>>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    ensure_quiz_variatif_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(quiz_id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {quiz_id} not found"),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let _grader_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    if let Some(Json(ref p)) = payload {
        if let Some(ref answer_grades) = p.answer_grades {
            for ag in answer_grades {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO attempt_answers (id, attempt_id, question_id, points_earned, is_correct, teacher_feedback, created_at)
                    VALUES (gen_random_uuid(), $3, $4, $1, ($1 > 0), NULLIF(TRIM($2), ''), NOW())
                    ON CONFLICT (attempt_id, question_id) DO UPDATE
                    SET points_earned = EXCLUDED.points_earned,
                        is_correct = EXCLUDED.is_correct,
                        teacher_feedback = COALESCE(EXCLUDED.teacher_feedback, attempt_answers.teacher_feedback)
                    "#,
                )
                .bind(ag.points_earned)
                .bind(&ag.teacher_feedback)
                .bind(attempt_id)
                .bind(ag.question_id)
                .execute(&ctx.pool)
                .await;
            }
        }

        let new_score: i32 = if let Some(manual_score) = p.score {
            manual_score
        } else {
            sqlx::query_scalar::<_, i32>(
                r#"
                SELECT COALESCE(SUM(ans.points_earned), 0)::int
                FROM attempt_answers ans
                JOIN quiz_attempts qa ON qa.id = ans.attempt_id
                WHERE ans.attempt_id = $1 AND qa.tenant_id = $2
                "#,
            )
            .bind(attempt_id)
            .bind(req_ctx.tenant_id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap_or(0)
        };

        let pass_info = sqlx::query(
            "SELECT q.passing_score, q.max_score, a.total_points FROM quizzes q JOIN quiz_attempts a ON a.quiz_id = q.id WHERE a.id = $1 AND a.tenant_id = $2 AND q.tenant_id = $2"
        )
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        let is_passed = if let Some(prow) = pass_info {
            let pass_score: i32 = prow.get("passing_score");
            let max_score: i32 = prow.get("max_score");
            let total_points: i32 = prow.get("total_points");
            let eff_max = if total_points > 0 {
                total_points
            } else if max_score > 0 {
                max_score
            } else {
                100
            };
            ((new_score * 100) / eff_max.max(1)).clamp(0, 100) >= pass_score
        } else {
            new_score >= 75
        };

        let _ = sqlx::query(
            r#"
            UPDATE quiz_attempts 
            SET score = $1, passed = $2, status = 'graded', completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
            WHERE id = $3 AND quiz_id = $4 AND tenant_id = $5
            "#
        )
        .bind(new_score)
        .bind(is_passed)
        .bind(attempt_id)
        .bind(quiz_id)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;
    } else {
        let command = GradeAttemptCommand { attempt_id };
        let _ = ctx.grade_attempt.execute(command).await;
    }

    // ── FCM update & sinkronisasi XP dari poin hasil penilaian kuis ke siswa ──
    {
        use sqlx::Row;
        if let Some(row) = sqlx::query(
            "SELECT q.tenant_id AS tenant_id, q.title AS title, a.student_id AS student_id, s.user_id AS user_id, st.full_name AS sname, a.score AS score FROM quiz_attempts a JOIN quizzes q ON q.id = a.quiz_id JOIN students s ON s.id = a.student_id JOIN students st ON st.id = a.student_id WHERE a.id = $1 AND a.tenant_id = $2 LIMIT 1",
        )
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        {
            let tid: Option<Uuid> = row.try_get("tenant_id").ok();
            let user_id: Option<Uuid> = row.try_get("user_id").ok();
            let qtitle: String = row.try_get("title").unwrap_or_else(|_| "Kuis".to_string());
            let score: Option<f64> = row.try_get::<Option<f64>, _>("score").ok().flatten()
                .or_else(|| row.try_get::<Option<i32>, _>("score").ok().flatten().map(|v| v as f64));
            let quiz_earned_points: i32 = sqlx::query_scalar::<_, i32>(
                "SELECT COALESCE(SUM(points_earned), 0)::int FROM attempt_answers WHERE attempt_id = $1",
            )
            .bind(attempt_id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap_or(0);
            let fallback_quiz_score = score.map(|v| v.round() as i32).unwrap_or(0).max(0);
            let xp_pts = if quiz_earned_points > 0 { quiz_earned_points } else { fallback_quiz_score };
            if let Ok(student_uuid) = row.try_get::<Uuid, _>("student_id") {
                let xp_desc = format!("Poin Kuis: {} ({} Poin)", qtitle, xp_pts);
                let _ = crate::presentation::gamification::controller::sync_scored_xp_for_student(
                    &ctx.pool,
                    tid.unwrap_or(req_ctx.tenant_id),
                    student_uuid,
                    "QUIZ_SCORE",
                    attempt_id,
                    xp_pts,
                    &xp_desc,
                )
                .await;
            }
            let t = format!("🏆 Nilai Keluar: {}", qtitle);
            let b = match score {
                Some(v) => format!("Nilai kuismu sudah dinilai: {}. Cek detail di aplikasi!", v),
                None => "Nilai kuismu sudah dinilai. Cek detail di aplikasi!".to_string(),
            };
            if let (Some(tid_v), Some(uid)) = (tid.or(Some(req_ctx.tenant_id)), user_id) {
                let _ = sqlx::query(
                    "INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'GRADE_UPDATE', 'in_app', 'grade', $5, FALSE, NOW())",
                )
                .bind(tid_v).bind(uid).bind(&t).bind(&b).bind(attempt_id)
                .execute(&ctx.pool).await;
            }

            let fcm_target = if let Some(uid) = user_id {
                crate::infrastructure::fcm::FcmTarget::User(uid)
            } else {
                crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()])
            };
            crate::infrastructure::fcm::trigger_fcm_push_targeted(
                t, b,
                crate::infrastructure::fcm::FcmCategory::Grade,
                attempt_id,
                fcm_target,
            );
        }
    }

    crate::presentation::learning::grade_sync::sync_quiz_attempt_to_gradebook(
        &ctx.pool,
        req_ctx.tenant_id,
        attempt_id,
    )
    .await;

    let updated_attempt = sqlx::query(
        r#"
        SELECT 
            a.id, a.quiz_id, a.student_id, a.started_at, a.completed_at,
            a.score, a.total_points, a.status, a.created_at, a.updated_at,
            COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
            COALESCE(s.nisn, u.username, '-') as student_nisn,
            q.max_score, q.passing_score
        FROM quiz_attempts a
        JOIN quizzes q ON q.id = a.quiz_id
        LEFT JOIN students s ON s.id = a.student_id OR s.user_id = a.student_id
        LEFT JOIN users u ON u.id = a.student_id OR u.id = s.user_id
        WHERE a.id = $1 AND a.tenant_id = $2
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let score: Option<i32> = updated_attempt.get("score");
    let total_points: i32 = updated_attempt.get("total_points");
    let max_score: i32 = updated_attempt.get("max_score");
    let passing_score: i32 = updated_attempt.get("passing_score");

    let effective_max = if total_points > 0 {
        total_points
    } else if max_score > 0 {
        max_score
    } else {
        100
    };

    let percentage = score.map(|s| (s * 100) / effective_max);
    let passed = percentage.map(|p| p >= passing_score);

    Ok(Json(ApiResponse::success(
        AttemptResponse {
            id: updated_attempt.get("id"),
            quiz_id: updated_attempt.get("quiz_id"),
            student_id: updated_attempt.get("student_id"),
            started_at: updated_attempt.get("started_at"),
            completed_at: updated_attempt.get("completed_at"),
            score,
            total_points: effective_max,
            percentage,
            passed,
            status: updated_attempt.get("status"),
            created_at: updated_attempt.get("created_at"),
            updated_at: updated_attempt.get("updated_at"),
            student_name: updated_attempt.get("student_name"),
            student_nisn: updated_attempt.get("student_nisn"),
            answers: Vec::new(),
            leave_violations_count: None,
            last_violation_at: None,
            last_heartbeat_at: None,
            last_device_platform: None,
        },
        req_ctx.request_id,
    )))
}

async fn auto_grade_attempts(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(quiz_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, title, passing_score, max_score FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(quiz_id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {quiz_id} not found"),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let qtitle: String = quiz_meta.get("title");
    let passing_score: i32 = quiz_meta.try_get("passing_score").unwrap_or(75);
    let max_score: i32 = quiz_meta.try_get("max_score").unwrap_or(100);

    let has_essay: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM quiz_questions WHERE quiz_id = $1 AND UPPER(COALESCE(question_type, '')) IN ('ESSAY', 'URAIAN'))",
    )
    .bind(quiz_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(false);

    let pending_attempts = sqlx::query(
        r#"
        SELECT qa.id, qa.student_id, qa.total_points,
               COALESCE(s.user_id, qa.student_id) AS user_id
        FROM quiz_attempts qa
        LEFT JOIN students s ON s.id = qa.student_id OR s.user_id = qa.student_id
        WHERE qa.quiz_id = $1
          AND qa.tenant_id = $2
          AND LOWER(qa.status) IN ('completed', 'submitted')
        "#,
    )
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    ensure_quiz_variatif_schema(&ctx.pool).await;

    let mut graded_count = 0usize;
    let mut objective_pregraded_count = 0usize;
    for att in pending_attempts {
        let attempt_id: Uuid = att.get("id");
        let student_id: Uuid = att.get("student_id");
        let total_points: i32 = att.try_get::<Option<i32>, _>("total_points").ok().flatten().unwrap_or(0);
        let user_id: Option<Uuid> = att.try_get("user_id").ok();

        // Auto-score single-choice PG answers from official quiz_choices keys
        score_single_choice_pg_answers(&ctx.pool, attempt_id).await;

        // Auto-score variatif objective formats (TRUE_FALSE_MATRIX, MULTIPLE_SELECT, SHORT_ANSWER, MATCHING, ORDERING)
        score_variatif_objective_answers(&ctx.pool, attempt_id).await;

        let new_score: i32 = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(SUM(points_earned), 0)::int FROM attempt_answers WHERE attempt_id = $1",
        )
        .bind(attempt_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(0);

        if has_essay {
            // Jika paket ujian berisi Essay, jangan tutup status sebagai 'graded' dan jangan sinkron ke Rapor dulu.
            // Simpan skor awal objektif sementara sambil mempertahankan status 'submitted' agar guru menilai Essay dulu.
            let _ = sqlx::query(
                r#"
                UPDATE quiz_attempts
                SET score = $1, passed = FALSE, status = 'submitted', completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
                WHERE id = $2 AND tenant_id = $3
                "#,
            )
            .bind(new_score)
            .bind(attempt_id)
            .bind(req_ctx.tenant_id)
            .execute(&ctx.pool)
            .await;

            objective_pregraded_count += 1;
            continue;
        }

        let eff_max = if total_points > 0 {
            total_points
        } else if max_score > 0 {
            max_score
        } else {
            100
        };
        let percentage = ((new_score * 100) / eff_max.max(1)).clamp(0, 100);
        let is_passed = percentage >= passing_score;

        let _ = sqlx::query(
            r#"
            UPDATE quiz_attempts
            SET score = $1, passed = $2, status = 'graded', completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
            WHERE id = $3 AND tenant_id = $4
            "#,
        )
        .bind(new_score)
        .bind(is_passed)
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;

        let xp_desc = format!("Poin Kuis: {} ({} Poin)", qtitle, new_score.max(0));
        let _ = crate::presentation::gamification::controller::sync_scored_xp_for_student(
            &ctx.pool,
            req_ctx.tenant_id,
            student_id,
            "QUIZ_SCORE",
            attempt_id,
            new_score.max(0),
            &xp_desc,
        )
        .await;

        if let Some(uid) = user_id {
            let notif_title = format!("🏆 Nilai Keluar: {}", qtitle);
            let notif_body = format!("Hasil CBT kamu telah dinilai: {} / 100.", percentage);
            let _ = sqlx::query(
                "INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'GRADE_UPDATE', 'in_app', 'grade', $5, FALSE, NOW())",
            )
            .bind(req_ctx.tenant_id)
            .bind(uid)
            .bind(&notif_title)
            .bind(&notif_body)
            .bind(attempt_id)
            .execute(&ctx.pool)
            .await;

            crate::infrastructure::fcm::trigger_fcm_push_targeted(
                notif_title,
                notif_body,
                crate::infrastructure::fcm::FcmCategory::Grade,
                attempt_id,
                crate::infrastructure::fcm::FcmTarget::User(uid),
            );
        }

        crate::presentation::learning::grade_sync::sync_quiz_attempt_to_gradebook(
            &ctx.pool,
            req_ctx.tenant_id,
            attempt_id,
        )
        .await;

        graded_count += 1;
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "quiz_id": quiz_id,
            "graded_count": graded_count,
            "objective_pregraded_count": objective_pregraded_count,
            "has_essay": has_essay
        }),
        req_ctx.request_id,
    )))
}

async fn get_attempts(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<AttemptResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {id} not found"),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    super::proctoring::ensure_proctoring_schema(&ctx.pool).await;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_student = !actor_is_teacher_or_admin(&req_ctx);

    let rows = if is_student {
        let student_db_id = crate::authorization_helpers::AuthorizationScope::resolve_student_id(
            &ctx.pool,
            req_ctx.tenant_id,
            actor_id,
        )
        .await
        .unwrap_or(None);

        sqlx::query(
            r#"
            SELECT 
                a.id, a.quiz_id, a.student_id, a.started_at, a.completed_at,
                a.score, a.total_points, a.status, a.created_at, a.updated_at,
                COALESCE(a.leave_violations_count, 0) as leave_violations_count,
                a.last_violation_at, a.last_heartbeat_at, a.last_device_platform,
                COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
                COALESCE(s.nisn, u.username, '-') as student_nisn,
                q.max_score, q.passing_score
            FROM quiz_attempts a
            JOIN quizzes q ON q.id = a.quiz_id
            LEFT JOIN students s ON s.id = a.student_id OR s.user_id = a.student_id
            LEFT JOIN users u ON u.id = a.student_id OR u.id = s.user_id
            WHERE a.quiz_id = $1 AND a.tenant_id = $2
              AND (
                  a.student_id = $3
                  OR s.user_id = $3
                  OR ($4::uuid IS NOT NULL AND a.student_id = $4)
              )
            ORDER BY a.started_at DESC
            "#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(student_db_id)
        .fetch_all(&ctx.pool)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT 
                a.id, a.quiz_id, a.student_id, a.started_at, a.completed_at,
                a.score, a.total_points, a.status, a.created_at, a.updated_at,
                COALESCE(a.leave_violations_count, 0) as leave_violations_count,
                a.last_violation_at, a.last_heartbeat_at, a.last_device_platform,
                COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
                COALESCE(s.nisn, u.username, '-') as student_nisn,
                q.max_score, q.passing_score
            FROM quiz_attempts a
            JOIN quizzes q ON q.id = a.quiz_id
            LEFT JOIN students s ON s.id = a.student_id OR s.user_id = a.student_id
            LEFT JOIN users u ON u.id = a.student_id OR u.id = s.user_id
            WHERE a.quiz_id = $1 AND a.tenant_id = $2
            ORDER BY a.started_at DESC
            "#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
    }
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let items: Vec<AttemptResponse> = rows
        .into_iter()
        .map(|r| {
            let (score, effective_max, percentage, passed) =
                build_attempt_score_summary(&r, !is_student);

            AttemptResponse {
                id: r.get("id"),
                quiz_id: r.get("quiz_id"),
                student_id: r.get("student_id"),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                score,
                total_points: effective_max,
                percentage,
                passed,
                status: r.get("status"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                student_name: r.get("student_name"),
                student_nisn: r.get("student_nisn"),
                answers: Vec::new(),
                leave_violations_count: r.try_get("leave_violations_count").ok(),
                last_violation_at: r.try_get("last_violation_at").ok().flatten(),
                last_heartbeat_at: r.try_get("last_heartbeat_at").ok().flatten(),
                last_device_platform: r.try_get("last_device_platform").ok().flatten(),
            }
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_attempt_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((quiz_id, attempt_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<AttemptResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(quiz_id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {quiz_id} not found"),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let row = sqlx::query(
        r#"
        SELECT 
            a.id, a.quiz_id, a.student_id, a.started_at, a.completed_at,
            a.score, a.total_points, a.status, a.created_at, a.updated_at,
            COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
            COALESCE(s.nisn, u.username, '-') as student_nisn,
            q.max_score, q.passing_score
        FROM quiz_attempts a
        JOIN quizzes q ON q.id = a.quiz_id
        LEFT JOIN students s ON s.id = a.student_id OR s.user_id = a.student_id
        LEFT JOIN users u ON u.id = a.student_id OR u.id = s.user_id
        WHERE a.id = $1 AND a.quiz_id = $2 AND a.tenant_id = $3
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .bind(quiz_id)
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
                school_core::common::error_code::ErrorCode::AttemptNotFound,
                format!("Attempt {} not found", attempt_id),
            ),
            &req_ctx.request_id,
        )
    })?;

    ensure_quiz_variatif_schema(&ctx.pool).await;
    let is_teacher_actor = actor_is_teacher_or_admin(&req_ctx);
    let attempt_student_id: Uuid = row.get("student_id");
    let attempt_status: String = row.get("status");
    let attempt_is_graded = attempt_status.eq_ignore_ascii_case("graded");

    if !is_teacher_actor {
        let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
        let owns_attempt = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT (
                $1 = $2
                OR EXISTS(SELECT 1 FROM students s WHERE s.id = $1 AND s.tenant_id = $3 AND (s.user_id = $2 OR s.id = $2))
                OR EXISTS(SELECT 1 FROM students s JOIN guardians g ON g.id = s.guardian_id WHERE s.id = $1 AND s.tenant_id = $3 AND g.user_id = $2)
            )
            "#,
        )
        .bind(attempt_student_id)
        .bind(actor_id)
        .bind(req_ctx.tenant_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(false);

        if !owns_attempt {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Anda tidak memiliki akses untuk melihat lembar jawaban siswa lain.".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let answer_rows = if is_teacher_actor {
        sqlx::query(
            r#"
            SELECT 
                q.id as question_id,
                q.question_text,
                q.question_type,
                q.points as max_points,
                q.interaction_config,
                ans.chosen_choice_id,
                c.choice_text as chosen_choice_text,
                ans.is_correct as ans_is_correct,
                c.is_correct as choice_is_correct,
                ans.text_answer,
                COALESCE(ans.points_earned, 0) as points_earned,
                ans.teacher_feedback
            FROM quiz_questions q
            LEFT JOIN attempt_answers ans ON ans.question_id = q.id AND ans.attempt_id = $1
            LEFT JOIN quiz_choices c ON c.id = ans.chosen_choice_id
            WHERE q.quiz_id = $2
            ORDER BY q.order_index ASC, q.created_at ASC
            "#,
        )
        .bind(attempt_id)
        .bind(quiz_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query(
            r#"
            SELECT 
                ans.question_id,
                q.question_text,
                q.question_type,
                q.points as max_points,
                q.interaction_config,
                ans.chosen_choice_id,
                c.choice_text as chosen_choice_text,
                ans.is_correct as ans_is_correct,
                c.is_correct as choice_is_correct,
                ans.text_answer,
                COALESCE(ans.points_earned, 0) as points_earned,
                ans.teacher_feedback
            FROM attempt_answers ans
            JOIN quiz_attempts qa ON qa.id = ans.attempt_id
            JOIN quiz_questions q ON q.id = ans.question_id
            LEFT JOIN quiz_choices c ON c.id = ans.chosen_choice_id
            WHERE ans.attempt_id = $1 AND qa.tenant_id = $2
            ORDER BY q.order_index ASC, q.created_at ASC
            "#,
        )
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    };

    let answers: Vec<AttemptAnswerDetailDto> = answer_rows
        .iter()
        .map(|a| build_attempt_answer_detail_dto(a, is_teacher_actor, attempt_is_graded))
        .collect();

    let (score, effective_max, percentage, passed) =
        build_attempt_score_summary(&row, is_teacher_actor);

    let resp = AttemptResponse {
        id: row.get("id"),
        quiz_id: row.get("quiz_id"),
        student_id: row.get("student_id"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        score,
        total_points: effective_max,
        percentage,
        passed,
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        student_name: row.get("student_name"),
        student_nisn: row.get("student_nisn"),
        answers,
        leave_violations_count: None,
        last_violation_at: None,
        last_heartbeat_at: None,
        last_device_platform: None,
    };

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_questions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<QuizQuestionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    ensure_quiz_variatif_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, status, release_at, start_at, exam_token FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
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
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {id} not found"),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let is_teacher = actor_is_teacher_or_admin(&req_ctx);
    if !is_teacher {
        let q_status: String = quiz_meta.try_get("status").unwrap_or_else(|_| "draft".to_string());
        let q_release: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("release_at").ok().flatten();
        let q_start: Option<chrono::DateTime<chrono::Utc>> = quiz_meta.try_get("start_at").ok().flatten();
        let q_token: Option<String> = quiz_meta.try_get("exam_token").ok().flatten();

        if (!q_status.eq_ignore_ascii_case("published") && !q_status.eq_ignore_ascii_case("active"))
            || q_release.or(q_start).is_some_and(|r| chrono::Utc::now() < r)
        {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Soal ujian belum dibuka untuk siswa.".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }

        if let Some(ref expected_tok) = q_token {
            if !expected_tok.trim().is_empty() {
                let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
                let effective_student_id =
                    crate::authorization_helpers::AuthorizationScope::resolve_student_id(
                        &ctx.pool,
                        req_ctx.tenant_id,
                        actor_id,
                    )
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(actor_id);

                let unlocked = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT (
                        EXISTS(
                            SELECT 1 FROM quiz_token_attempts
                            WHERE tenant_id = $1 AND quiz_id = $2 AND student_id = $3
                              AND failed_attempts = 0 AND last_attempt_at IS NOT NULL
                              AND (locked_until IS NULL OR locked_until <= NOW())
                        )
                        OR EXISTS(
                            SELECT 1 FROM quiz_attempts
                            WHERE tenant_id = $1 AND quiz_id = $2 AND student_id = $3
                        )
                    )
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(id)
                .bind(effective_student_id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap_or(false);

                if !unlocked {
                    return Err(ApiError::new(
                        school_core::common::error::ApplicationError::Domain(
                            school_core::common::error::DomainError::Validation(
                                "Token ujian wajib diverifikasi sebelum membuka soal ujian.".to_string(),
                            ),
                        ),
                        &req_ctx.request_id,
                    ));
                }
            }
        }
    }

    #[derive(sqlx::FromRow)]
    struct QuestionRow {
        id: Uuid,
        question_text: String,
        question_type: String,
        points: i32,
        order_index: i32,
        interaction_config: Option<serde_json::Value>,
    }

    #[derive(sqlx::FromRow)]
    struct ChoiceRow {
        id: Uuid,
        choice_text: String,
        order_index: i32,
        is_correct: bool,
    }

    let questions: Vec<QuestionRow> = sqlx::query_as(
        r#"
        SELECT q.id, q.question_text, q.question_type, q.points, q.order_index, q.interaction_config
        FROM quiz_questions q
        JOIN quizzes qz ON qz.id = q.quiz_id
        WHERE q.quiz_id = $1 AND qz.tenant_id = $2
        ORDER BY q.order_index ASC
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
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

    let mut result = Vec::new();
    for q in questions {
        let choices: Vec<ChoiceRow> = sqlx::query_as(
            r#"
            SELECT id, choice_text, order_index, is_correct
            FROM quiz_choices
            WHERE question_id = $1
            ORDER BY order_index ASC
            "#,
        )
        .bind(q.id)
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

        let extracted_image = extract_embedded_image_url(&q.question_text);
        let safe_config = if is_teacher {
            q.interaction_config
        } else {
            sanitize_interaction_config_for_student(&q.question_type, q.id, q.interaction_config)
        };
        result.push(QuizQuestionResponse {
            id: q.id,
            question_text: q.question_text,
            question_type: q.question_type,
            points: q.points,
            order_index: q.order_index,
            image_url: extracted_image,
            choices: choices
                .into_iter()
                .map(|c| QuizChoiceResponse {
                    id: c.id,
                    choice_text: c.choice_text,
                    order_index: c.order_index,
                    is_correct: if is_teacher { Some(c.is_correct) } else { None },
                })
                .collect(),
            interaction_config: safe_config,
        });
    }

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

async fn add_question(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateQuizQuestionRequest>,
) -> Result<Json<ApiResponse<QuizQuestionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let quiz_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL)",
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
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

    if !quiz_exists {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::QuizNotFound,
                format!("Quiz {} not found", id),
            ),
            &req_ctx.request_id,
        ));
    }

    ensure_quiz_variatif_schema(&ctx.pool).await;

    let q_id = Uuid::now_v7();
    let q_type = payload
        .question_type
        .unwrap_or_else(|| "MULTIPLE_CHOICE".to_string());
    let points = payload.points.unwrap_or(10);
    let order_idx = payload.order_index.unwrap_or(1);
    let stored_question_text =
        merge_question_image_into_text(&payload.question_text, payload.image_url.as_deref());

    sqlx::query(
        r#"
        INSERT INTO quiz_questions (id, quiz_id, question_text, question_type, points, order_index, interaction_config, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
        "#,
    )
    .bind(q_id)
    .bind(id)
    .bind(&stored_question_text)
    .bind(&q_type)
    .bind(points)
    .bind(order_idx)
    .bind(&payload.interaction_config)
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let mut choice_responses = Vec::new();
    if let Some(choices) = payload.choices {
        if !choices.is_empty() {
            let choice_count = choices.len();
            let mut choice_ids = Vec::with_capacity(choice_count);
            let mut choice_texts = Vec::with_capacity(choice_count);
            let mut is_corrects = Vec::with_capacity(choice_count);
            let mut order_indexes = Vec::with_capacity(choice_count);

            for (idx, c) in choices.into_iter().enumerate() {
                choice_ids.push(Uuid::now_v7());
                choice_texts.push(c.choice_text);
                is_corrects.push(c.is_correct.unwrap_or(false));
                order_indexes.push(c.order_index.unwrap_or(idx as i32 + 1));
            }

            let q_ids = vec![q_id; choice_count];
            sqlx::query(
                r#"
                INSERT INTO quiz_choices (id, question_id, choice_text, is_correct, order_index, created_at, updated_at)
                SELECT u.id, u.question_id, u.choice_text, u.is_correct, u.order_index, NOW(), NOW()
                FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::boolean[], $5::int[])
                    AS u(id, question_id, choice_text, is_correct, order_index)
                "#
            )
            .bind(&choice_ids as &[Uuid])
            .bind(&q_ids as &[Uuid])
            .bind(&choice_texts as &[String])
            .bind(&is_corrects as &[bool])
            .bind(&order_indexes as &[i32])
            .execute(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e)
            ), &req_ctx.request_id))?;

            for (idx, c_id) in choice_ids.into_iter().enumerate() {
                choice_responses.push(QuizChoiceResponse {
                    id: c_id,
                    choice_text: choice_texts[idx].clone(),
                    order_index: order_indexes[idx],
                    is_correct: Some(is_corrects[idx]),
                });
            }
        }
    }

    let _ = sqlx::query(
        r#"
        UPDATE quizzes 
        SET questions_count = (SELECT COUNT(*) FROM quiz_questions WHERE quiz_id = $1),
            max_score = (SELECT COALESCE(SUM(points), 0) FROM quiz_questions WHERE quiz_id = $1)
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await;

    Ok(Json(ApiResponse::success(
        QuizQuestionResponse {
            id: q_id,
            question_text: stored_question_text,
            question_type: q_type,
            points,
            order_index: order_idx,
            image_url: payload.image_url,
            choices: choice_responses,
            interaction_config: payload.interaction_config,
        },
        req_ctx.request_id,
    )))
}

fn build_attempt_answer_detail_dto(
    a: &sqlx::postgres::PgRow,
    is_teacher_actor: bool,
    attempt_is_graded: bool,
) -> AttemptAnswerDetailDto {
    let q_id: Uuid = a.get("question_id");
    let q_type: String = a.get("question_type");
    let raw_cfg: Option<serde_json::Value> = a.try_get("interaction_config").ok().flatten();
    let safe_cfg = if is_teacher_actor {
        raw_cfg
    } else {
        sanitize_interaction_config_for_student(&q_type, q_id, raw_cfg)
    };
    let ans_is_correct: Option<bool> = a.try_get("ans_is_correct").ok().flatten();
    let choice_is_correct: Option<bool> = a.try_get("choice_is_correct").ok().flatten();
    let exposed_is_correct = if is_teacher_actor {
        ans_is_correct.or(choice_is_correct)
    } else if attempt_is_graded {
        ans_is_correct
    } else {
        None
    };
    let exposed_points = if is_teacher_actor || attempt_is_graded {
        a.try_get::<Option<i32>, _>("points_earned")
            .ok()
            .flatten()
            .unwrap_or(0)
    } else {
        0
    };
    let exposed_feedback = if is_teacher_actor || attempt_is_graded {
        a.try_get::<Option<String>, _>("teacher_feedback")
            .ok()
            .flatten()
    } else {
        None
    };
    AttemptAnswerDetailDto {
        question_id: q_id,
        question_text: a.get("question_text"),
        question_type: q_type,
        max_points: a.get("max_points"),
        chosen_choice_id: a.get("chosen_choice_id"),
        chosen_choice_text: a.get("chosen_choice_text"),
        is_correct: exposed_is_correct,
        text_answer: a.get("text_answer"),
        points_earned: exposed_points,
        teacher_feedback: exposed_feedback,
        interaction_config: safe_cfg,
    }
}

/// Withholds provisional partial scores (e.g. objective pre-grades while essay grading is still pending)
/// from non-staff callers until the attempt status is finalized as `graded`.
fn mask_ungraded_score(
    raw_score: Option<i32>,
    status: Option<&str>,
    is_staff_actor: bool,
) -> Option<i32> {
    if is_staff_actor || status.is_some_and(|s| s.eq_ignore_ascii_case("graded")) {
        raw_score
    } else {
        None
    }
}

/// Computes `(visible_score, effective_max, percentage, passed)` while withholding
/// unfinalized scores from non-staff actors.
pub(super) fn build_attempt_score_summary(
    row: &sqlx::postgres::PgRow,
    is_staff_actor: bool,
) -> (Option<i32>, i32, Option<i32>, Option<bool>) {
    let status: String = row.get("status");
    let raw_score: Option<i32> = row.get("score");
    let total_points: i32 = row.get("total_points");
    let max_score: i32 = row.get("max_score");
    let passing_score: i32 = row.get("passing_score");

    let effective_max = if total_points > 0 {
        total_points
    } else if max_score > 0 {
        max_score
    } else {
        100
    };

    let visible_score = mask_ungraded_score(raw_score, Some(status.as_str()), is_staff_actor);
    let percentage = visible_score.map(|s| (s * 100) / effective_max);
    let passed = percentage.map(|p| p >= passing_score);

    (visible_score, effective_max, percentage, passed)
}

pub(super) fn actor_is_admin_or_principal(req_ctx: &RequestContext) -> bool {
    req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin")
                    || n.contains("kepala")
                    || n.contains("principal")
                    || n.contains("operator")
                    || n.contains("staff")
                    || n.contains("staf")
                    || n == "tu"
            })
        })
        .unwrap_or(false)
}

pub(super) fn actor_is_teacher_or_admin(req_ctx: &RequestContext) -> bool {
    req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("guru")
                    || n.contains("teacher")
                    || n.contains("pengajar")
                    || n.contains("tutor")
                    || n.contains("pamong")
                    || n.contains("kepala")
                    || n.contains("principal")
                    || n.contains("admin")
                    || n.contains("operator")
                    || n.contains("staff")
                    || n.contains("staf")
                    || n == "tu"
            })
        })
        .unwrap_or(false)
}

fn build_quiz_response_from_list_row(
    r: &sqlx::postgres::PgRow,
    hide_token: bool,
    include_student_attempt: bool,
    now_utc: chrono::DateTime<chrono::Utc>,
) -> QuizResponse {
    let dur = r.get::<Option<i32>, _>("time_limit_minutes").unwrap_or(30);
    let raw_token: Option<String> = r.try_get("exam_token").ok().flatten();
    let req_tok = raw_token.as_deref().is_some_and(|t| !t.trim().is_empty());

    let (
        student_attempt_status,
        student_attempts_count,
        student_has_completed,
        student_last_score,
        student_last_attempt_id,
    ) = if include_student_attempt {
        let att_status: Option<String> = r.try_get("student_attempt_status").ok().flatten();
        let att_count: Option<i32> = r.try_get("student_attempts_count").ok().flatten();
        let raw_score: Option<i32> = r.try_get("student_last_score").ok().flatten();
        let score = mask_ungraded_score(raw_score, att_status.as_deref(), false);
        let att_id: Option<Uuid> = r.try_get("student_last_attempt_id").ok().flatten();
        let max_att = r.get::<i32, _>("max_attempts");
        let count_val = att_count.unwrap_or(0);
        let is_completed = att_status
            .as_deref()
            .map(|s| {
                let s_lower = s.to_lowercase();
                s_lower == "completed" || s_lower == "submitted" || s_lower == "graded"
            })
            .unwrap_or(false)
            || (count_val > 0 && count_val >= max_att);

        (att_status, att_count, Some(is_completed), score, att_id)
    } else {
        (None, None, None, None, None)
    };

    let mut resp = QuizResponse {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        lesson_id: r.get("lesson_id"),
        title: r.get("title"),
        description: r.get("description"),
        duration_minutes: dur,
        time_limit_minutes: dur,
        passing_score: r.get("passing_score"),
        max_score: r.get("max_score"),
        max_attempts: r.get("max_attempts"),
        shuffle_questions: r.get("shuffle_questions"),
        shuffle_choices: r.get("shuffle_choices"),
        start_at: r.get("start_at"),
        end_at: r.get("end_at"),
        status: r.get("status"),
        questions_count: r.get("questions_count"),
        is_active: r.get("is_active"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        class_id: r.get("class_id"),
        class_name: r.get("class_name"),
        subject_name: r.get("subject_name"),
        teacher_id: r.try_get("teacher_id").ok().flatten(),
        teacher_name: r.get("teacher_name"),
        session_id: r.get("session_id"),
        release_at: r
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at")
            .ok()
            .flatten(),
        exam_mode: r
            .get::<Option<String>, _>("exam_mode")
            .or_else(|| Some("HOMEWORK_QUIZ".to_string())),
        assessment_category: r
            .try_get::<Option<String>, _>("assessment_category")
            .ok()
            .flatten()
            .or_else(|| Some("FORMATIVE".to_string())),
        exam_token: if hide_token { None } else { raw_token },
        requires_token: req_tok,
        token_expires_at: r.get("token_expires_at"),
        max_token_attempts: r.get::<Option<i32>, _>("max_token_attempts").or(Some(5)),
        student_attempt_status,
        student_attempts_count,
        student_has_completed,
        student_last_score,
        student_last_attempt_id,
        exam_phase: "DRAFT".to_string(),
        is_open_for_student: false,
        seconds_until_start: None,
        seconds_remaining: None,
    };
    resp.apply_schedule_evaluation(now_utc);
    resp
}

pub(super) async fn score_single_choice_pg_answers(pool: &sqlx::PgPool, attempt_id: Uuid) {
    let _ = sqlx::query(
        r#"
        UPDATE attempt_answers ans
        SET points_earned = CASE WHEN COALESCE(c.is_correct, false) THEN COALESCE(q.points, 10) ELSE 0 END,
            is_correct = COALESCE(c.is_correct, false)
        FROM quiz_questions q
        LEFT JOIN quiz_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.attempt_id = $1
          AND q.id = ans.question_id
          AND UPPER(COALESCE(q.question_type, 'MULTIPLE_CHOICE')) IN ('MULTIPLE_CHOICE', 'PG')
        "#,
    )
    .bind(attempt_id)
    .execute(pool)
    .await;
}

fn actor_is_strict_teacher(req_ctx: &RequestContext) -> bool {
    req_ctx
        .actor
        .as_ref()
        .map(|a| {
            let has_admin = a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("admin")
                    || n.contains("kepala")
                    || n.contains("principal")
                    || n.contains("operator")
                    || n.contains("staff")
                    || n.contains("staf")
                    || n == "tu"
            });
            let has_teacher = a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("guru")
                    || n.contains("teacher")
                    || n.contains("pengajar")
                    || n.contains("tutor")
                    || n.contains("pamong")
            });
            has_teacher && !has_admin
        })
        .unwrap_or(false)
}

fn is_disallowed_svg_or_placeholder(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("unsplash.com") || lower.contains(".svg") || lower.contains("/ai_diagram_")
}

fn merge_question_image_into_text(question_text: &str, image_url: Option<&str>) -> String {
    let trimmed_text = question_text.trim();
    if let Some(raw_img) = image_url {
        let img = raw_img.trim();
        if !img.is_empty()
            && !is_disallowed_svg_or_placeholder(img)
            && !trimmed_text.contains("![")
        {
            if trimmed_text.is_empty() {
                return format!("![Stimulus Gambar]({img})");
            }
            return format!("![Stimulus Gambar]({img})\n\n{trimmed_text}");
        }
    }
    trimmed_text.to_string()
}

fn extract_embedded_image_url(question_text: &str) -> Option<String> {
    let start = question_text.find("![")?;
    let after_alt = question_text[start..].find("](")?;
    let url_start = start + after_alt + 2;
    let url_end = question_text[url_start..].find(')')?;
    let candidate = question_text[url_start..url_start + url_end].trim();
    if candidate.is_empty() || is_disallowed_svg_or_placeholder(candidate) {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn deterministic_shuffle_with_seed<T>(mut items: Vec<T>, seed_str: &str) -> Vec<T> {
    if items.len() <= 1 {
        return items;
    }
    let mut seed: u64 = 14695981039346656037;
    for b in seed_str.as_bytes() {
        seed ^= u64::from(*b);
        seed = seed.wrapping_mul(1099511628211);
    }
    for i in (1..items.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = ((seed >> 33) as usize) % (i + 1);
        items.swap(i, j);
    }
    items
}

fn sanitize_interaction_config_for_student(
    question_type: &str,
    question_id: Uuid,
    raw: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let cfg = raw?;
    let obj = cfg.as_object()?;
    let norm = question_type.trim().to_uppercase();

    if norm == "SHORT_ANSWER" || norm == "ISIAN_SINGKAT" || norm == "FILL_BLANK" {
        let mut out = serde_json::Map::new();
        out.insert("has_answer_key".to_string(), serde_json::Value::Bool(true));
        if let Some(ph) = obj.get("placeholder") {
            out.insert("placeholder".to_string(), ph.clone());
        }
        return Some(serde_json::Value::Object(out));
    }

    if norm == "MATCHING" || norm == "MENJODOHKAN" {
        let pairs = obj
            .get("pairs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let left_items: Vec<serde_json::Value> = pairs
            .iter()
            .enumerate()
            .filter_map(|(idx, p)| {
                let id = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("pair-{}", idx + 1));
                let left = p.get("left").and_then(|v| v.as_str())?.trim();
                if left.is_empty() {
                    return None;
                }
                Some(serde_json::json!({ "id": id, "text": left }))
            })
            .collect();

        let mut rights: Vec<String> = pairs
            .iter()
            .filter_map(|p| p.get("right").and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(distractors) = obj.get("distractors").and_then(|v| v.as_array()) {
            for d in distractors {
                if let Some(s) = d.as_str() {
                    let clean = s.trim().to_string();
                    if !clean.is_empty() && !rights.contains(&clean) {
                        rights.push(clean);
                    }
                }
            }
        }
        let shuffled_rights =
            deterministic_shuffle_with_seed(rights, &format!("match:{question_id}"));
        return Some(serde_json::json!({
            "left_items": left_items,
            "right_options": shuffled_rights
        }));
    }

    if norm == "ORDERING" || norm == "URUTAN" || norm == "SEQUENCE" {
        let items: Vec<serde_json::Value> = obj
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .filter_map(|(idx, item)| {
                        let id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("step-{}", idx + 1));
                        let text = item.get("text").and_then(|v| v.as_str())?.trim();
                        if text.is_empty() {
                            return None;
                        }
                        Some(serde_json::json!({ "id": id, "text": text }))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let shuffled =
            deterministic_shuffle_with_seed(items, &format!("order:{question_id}"));
        let mut out = serde_json::Map::new();
        out.insert("items".to_string(), serde_json::Value::Array(shuffled));
        if let Some(inst) = obj.get("instruction") {
            out.insert("instruction".to_string(), inst.clone());
        }
        return Some(serde_json::Value::Object(out));
    }

    if norm == "TRUE_FALSE_MATRIX" || norm == "TRUE_FALSE" || norm == "BENAR_SALAH" {
        let mut out = serde_json::Map::new();
        for key in ["statement_label", "true_label", "false_label"] {
            if let Some(val) = obj.get(key) {
                out.insert(key.to_string(), val.clone());
            }
        }
        return Some(serde_json::Value::Object(out));
    }

    if norm == "ESSAY" || norm == "URAIAN" {
        return None;
    }

    Some(cfg)
}

pub(super) async fn score_variatif_objective_answers(pool: &sqlx::PgPool, attempt_id: Uuid) {
    let rows = match sqlx::query(
        r#"
        SELECT
            ans.question_id,
            q.question_type,
            COALESCE(q.points, 10) AS max_points,
            q.interaction_config,
            ans.text_answer
        FROM attempt_answers ans
        JOIN quiz_questions q ON q.id = ans.question_id
        WHERE ans.attempt_id = $1
          AND UPPER(COALESCE(q.question_type, 'MULTIPLE_CHOICE')) NOT IN ('MULTIPLE_CHOICE', 'PG', 'ESSAY', 'URAIAN')
        "#,
    )
    .bind(attempt_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => return,
    };

    for row in rows {
        let question_id: Uuid = row.get("question_id");
        let q_type: String = row.get("question_type");
        let max_points: i32 = row.get("max_points");
        let interaction_cfg: Option<serde_json::Value> =
            row.try_get("interaction_config").ok().flatten();
        let text_answer: String = row
            .try_get::<Option<String>, _>("text_answer")
            .ok()
            .flatten()
            .unwrap_or_default();

        let norm = q_type.trim().to_uppercase();
        let mut computed: Option<(i32, bool)> = None;

        if matches!(
            norm.as_str(),
            "TRUE_FALSE_MATRIX" | "TRUE_FALSE" | "BENAR_SALAH" | "MATRIX_TF"
        ) {
            if let Ok(choice_rows) = sqlx::query(
                "SELECT id, is_correct FROM quiz_choices WHERE question_id = $1 ORDER BY order_index ASC",
            )
            .bind(question_id)
            .fetch_all(pool)
            .await
            {
                if !choice_rows.is_empty() {
                    let parsed = serde_json::from_str::<serde_json::Value>(&text_answer).ok();
                    let row_map = parsed
                        .as_ref()
                        .and_then(|v| v.get("rows").or(Some(v)))
                        .and_then(|v| v.as_object());
                    let mut correct_rows = 0usize;
                    for (idx, cr) in choice_rows.iter().enumerate() {
                        let cid: Uuid = cr.get("id");
                        let expected: bool = cr.get("is_correct");
                        let cid_str = cid.to_string();
                        let alt_idx = (idx + 1).to_string();
                        let pick = row_map.and_then(|m| {
                            m.get(&cid_str)
                                .or_else(|| m.get(&alt_idx))
                                .and_then(|v| v.as_bool())
                        });
                        if pick == Some(expected) {
                            correct_rows += 1;
                        }
                    }
                    let ratio = (correct_rows as f64) / (choice_rows.len() as f64);
                    let pts = ((max_points as f64) * ratio).round() as i32;
                    computed = Some((pts, correct_rows == choice_rows.len()));
                }
            }
        } else if matches!(
            norm.as_str(),
            "MULTIPLE_SELECT" | "MULTI_SELECT" | "PG_KOMPLEKS" | "CHECKBOX"
        ) {
            if let Ok(choice_rows) =
                sqlx::query("SELECT id, is_correct FROM quiz_choices WHERE question_id = $1")
                    .bind(question_id)
                    .fetch_all(pool)
                    .await
            {
                let correct_ids: std::collections::HashSet<String> = choice_rows
                    .iter()
                    .filter(|r| r.get::<bool, _>("is_correct"))
                    .map(|r| r.get::<Uuid, _>("id").to_string())
                    .collect();

                if !correct_ids.is_empty() {
                    let parsed = serde_json::from_str::<serde_json::Value>(&text_answer).ok();
                    let picks: std::collections::HashSet<String> = parsed
                        .as_ref()
                        .and_then(|v| v.get("choice_ids").or(Some(v)))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let mut tp = 0i32;
                    let mut fp = 0i32;
                    for id in &picks {
                        if correct_ids.contains(id) {
                            tp += 1;
                        } else {
                            fp += 1;
                        }
                    }
                    let ratio = ((tp - fp) as f64 / (correct_ids.len() as f64)).clamp(0.0, 1.0);
                    let pts = ((max_points as f64) * ratio).round() as i32;
                    computed = Some((pts, tp as usize == correct_ids.len() && fp == 0));
                }
            }
        } else if matches!(norm.as_str(), "SHORT_ANSWER" | "ISIAN_SINGKAT" | "FILL_BLANK") {
            if let Some(obj) = interaction_cfg.as_ref().and_then(|v| v.as_object()) {
                let accepted: Vec<String> = obj
                    .get("accepted_answers")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                if !accepted.is_empty() {
                    let case_sensitive = obj
                        .get("case_sensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let tolerance = obj
                        .get("numeric_tolerance")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0001)
                        .max(0.0);

                    let normalize = |raw: &str| -> String {
                        let collapsed = raw
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                            .to_string();
                        if case_sensitive {
                            collapsed
                        } else {
                            collapsed.to_lowercase()
                        }
                    };
                    let student_norm = normalize(&text_answer);
                    let student_num = text_answer.trim().replace(',', ".").parse::<f64>().ok();

                    let matched = !student_norm.is_empty()
                        && accepted.iter().any(|ans| {
                            if normalize(ans) == student_norm {
                                return true;
                            }
                            if let Some(sn) = student_num {
                                if let Ok(an) = ans.trim().replace(',', ".").parse::<f64>() {
                                    return (sn - an).abs() <= tolerance;
                                }
                            }
                            false
                        });
                    computed = Some((if matched { max_points } else { 0 }, matched));
                }
            }
        } else if matches!(norm.as_str(), "MATCHING" | "MENJODOHKAN") {
            if let Some(pairs) = interaction_cfg
                .as_ref()
                .and_then(|v| v.get("pairs"))
                .and_then(|v| v.as_array())
            {
                if !pairs.is_empty() {
                    let parsed = serde_json::from_str::<serde_json::Value>(&text_answer).ok();
                    let student_map = parsed
                        .as_ref()
                        .and_then(|v| v.get("pairs").or(Some(v)))
                        .and_then(|v| v.as_object());

                    let mut matched = 0usize;
                    for (idx, p) in pairs.iter().enumerate() {
                        let pid = p
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("pair-{}", idx + 1));
                        let expected_right = p
                            .get("right")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_lowercase();
                        let student_right = student_map
                            .and_then(|m| m.get(&pid))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_lowercase();
                        if !student_right.is_empty() && student_right == expected_right {
                            matched += 1;
                        }
                    }
                    let ratio = (matched as f64) / (pairs.len() as f64);
                    let pts = ((max_points as f64) * ratio).round() as i32;
                    computed = Some((pts, matched == pairs.len()));
                }
            }
        } else if matches!(norm.as_str(), "ORDERING" | "URUTAN" | "SEQUENCE") {
            if let Some(items) = interaction_cfg
                .as_ref()
                .and_then(|v| v.get("items"))
                .and_then(|v| v.as_array())
            {
                if !items.is_empty() {
                    let mut canonical: Vec<(i64, String)> = items
                        .iter()
                        .enumerate()
                        .map(|(idx, it)| {
                            let order = it
                                .get("correct_order")
                                .and_then(|v| v.as_i64())
                                .unwrap_or((idx + 1) as i64);
                            let id = it
                                .get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| format!("step-{}", idx + 1));
                            (order, id)
                        })
                        .collect();
                    canonical.sort_by_key(|(o, _)| *o);

                    let parsed = serde_json::from_str::<serde_json::Value>(&text_answer).ok();
                    let student_order: Vec<String> = parsed
                        .as_ref()
                        .and_then(|v| v.get("order").or(Some(v)))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let matched = canonical
                        .iter()
                        .enumerate()
                        .filter(|(idx, (_, expected_id))| {
                            student_order.get(*idx) == Some(expected_id)
                        })
                        .count();
                    let ratio = (matched as f64) / (canonical.len() as f64);
                    let pts = ((max_points as f64) * ratio).round() as i32;
                    computed = Some((pts, matched == canonical.len()));
                }
            }
        }

        if let Some((pts, is_ok)) = computed {
            let _ = sqlx::query(
                r#"
                UPDATE attempt_answers
                SET points_earned = $1, is_correct = $2
                WHERE attempt_id = $3 AND question_id = $4
                "#,
            )
            .bind(pts)
            .bind(is_ok)
            .bind(attempt_id)
            .bind(question_id)
            .execute(pool)
            .await;
        }
    }
}
