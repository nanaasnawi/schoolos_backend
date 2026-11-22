use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::Row;
use tokio::sync::OnceCell;
use uuid::Uuid;

use super::controller::{
    actor_is_admin_or_principal, actor_is_teacher_or_admin, ensure_quiz_variatif_schema, get_by_id,
    score_single_choice_pg_answers, score_variatif_objective_answers,
};
use super::dto::proctoring_dto::{
    ExtendTimeRequest, ProctorEventDto, ProctorMonitorResponse, ProctorMonitorSummaryDto,
    ProctorParticipantRowDto, ProctorScheduleConflictDto, ProctorScheduleDashboardResponse,
    ProctorScheduleSummaryDto, ProctorSessionOverviewDto, RecordProctorEventRequest,
    RotateTokenRequest,
};
use super::dto::quiz_response::QuizResponse;
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::domain::proctoring::{
    classify_proctor_participant_status, compute_extended_end_at,
    detect_proctor_schedule_conflicts, evaluate_quiz_schedule, generate_ambiguity_free_exam_token,
    is_violation_event_type, ExamPhase, ProctorParticipantStatus, QuizScheduleInput,
    ScheduleConflictInput,
};
use school_core::permission::domain::permission_registry::Permission;

static PROCTOR_SCHEMA_READY: OnceCell<()> = OnceCell::const_new();

/// Ensures proctoring telemetry columns and audit table exist idempotently.
pub async fn ensure_proctoring_schema(pool: &sqlx::PgPool) {
    PROCTOR_SCHEMA_READY
        .get_or_init(|| async {
            let statements = [
                "ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS leave_violations_count INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_violation_at TIMESTAMP WITH TIME ZONE",
                "ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMP WITH TIME ZONE",
                "ALTER TABLE quiz_attempts ADD COLUMN IF NOT EXISTS last_device_platform VARCHAR(32)",
                r#"
                CREATE TABLE IF NOT EXISTS quiz_proctor_events (
                    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    quiz_id UUID NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
                    attempt_id UUID NOT NULL REFERENCES quiz_attempts(id) ON DELETE CASCADE,
                    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
                    event_type VARCHAR(40) NOT NULL,
                    detail TEXT,
                    device_platform VARCHAR(32),
                    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
                )
                "#,
                "CREATE INDEX IF NOT EXISTS idx_quiz_proctor_events_quiz ON quiz_proctor_events(tenant_id, quiz_id, created_at DESC)",
                "CREATE INDEX IF NOT EXISTS idx_quiz_proctor_events_attempt ON quiz_proctor_events(attempt_id, created_at DESC)",
            ];
            for sql in statements {
                if let Err(e) = sqlx::query(sql).execute(pool).await {
                    tracing::warn!("ensure_proctoring_schema notice on `{sql}`: {e}");
                }
            }
        })
        .await;
}

fn db_err(e: sqlx::Error, request_id: &str) -> ApiError {
    ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e),
        ),
        request_id,
    )
}

fn forbidden_err(msg: &str, request_id: &str) -> ApiError {
    ApiError::new(
        school_core::common::error::ApplicationError::Unauthorized(
            school_core::common::error_code::ErrorCode::AuthPermissionDenied,
            msg.to_string(),
        ),
        request_id,
    )
}

fn not_found_err(msg: &str, request_id: &str) -> ApiError {
    ApiError::new(
        school_core::common::error::ApplicationError::NotFound(
            school_core::common::error_code::ErrorCode::QuizNotFound,
            msg.to_string(),
        ),
        request_id,
    )
}

fn validation_err(msg: &str, request_id: &str) -> ApiError {
    ApiError::new(
        school_core::common::error::ApplicationError::Domain(
            school_core::common::error::DomainError::Validation(msg.to_string()),
        ),
        request_id,
    )
}

fn require_staff_permission(
    req_ctx: &RequestContext,
    permission: Permission,
) -> Result<(), ApiError> {
    crate::middleware::require_permission(&req_ctx.actor, permission)
        .map_err(|_| forbidden_err("Insufficient permissions", &req_ctx.request_id))?;

    if !actor_is_teacher_or_admin(req_ctx) {
        return Err(forbidden_err(
            "Hanya Kepala Sekolah, Guru, atau Proktor yang dapat mengakses monitor ujian.",
            &req_ctx.request_id,
        ));
    }
    Ok(())
}

/// Returns a unified backend-evaluated CBT proctor schedule dashboard, including
/// class/teacher overlap conflicts and real-time per-session participant counts.
pub async fn get_proctor_schedule(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<ProctorScheduleDashboardResponse>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizRead)?;
    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    ensure_proctoring_schema(&ctx.pool).await;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_admin = actor_is_admin_or_principal(&req_ctx);
    let scoped_teacher_only = !is_admin;

    let rows = sqlx::query(
        r#"
        WITH latest_attempts AS (
            SELECT DISTINCT ON (qa.quiz_id, COALESCE(s.id, qa.student_id))
                qa.quiz_id,
                qa.status,
                COALESCE(qa.leave_violations_count, 0) AS violations
            FROM quiz_attempts qa
            LEFT JOIN students s
                ON s.tenant_id = qa.tenant_id
               AND (s.id = qa.student_id OR s.user_id = qa.student_id)
            WHERE qa.tenant_id = $1
            ORDER BY qa.quiz_id, COALESCE(s.id, qa.student_id), qa.created_at DESC
        ),
        attempt_summary AS (
            SELECT
                quiz_id,
                COUNT(*)::bigint AS started_count,
                COUNT(*) FILTER (WHERE LOWER(COALESCE(status, '')) IN ('created', 'started', 'in_progress'))::bigint AS in_progress_count,
                COUNT(*) FILTER (WHERE LOWER(COALESCE(status, '')) IN ('completed', 'submitted', 'auto_graded'))::bigint AS need_grading_count,
                COUNT(*) FILTER (WHERE LOWER(COALESCE(status, '')) IN ('graded', 'reviewed'))::bigint AS completed_count,
                COALESCE(SUM(violations), 0)::bigint AS total_violations,
                COUNT(*) FILTER (WHERE violations > 0)::bigint AS flagged_students_count
            FROM latest_attempts
            GROUP BY quiz_id
        ),
        class_roster_counts AS (
            SELECT
                en.class_id,
                COUNT(DISTINCT s.id)::bigint AS enrolled_count
            FROM enrollments en
            JOIN students s ON s.id = en.student_id
            WHERE s.tenant_id = $1
              AND s.deleted_at IS NULL
              AND (en.status ILIKE 'active' OR en.status IS NULL)
            GROUP BY en.class_id
        )
        SELECT
            q.id,
            q.title,
            q.status,
            q.is_active,
            q.questions_count,
            COALESCE(q.time_limit_minutes, 30) AS duration_minutes,
            q.passing_score,
            q.class_id,
            c.name AS class_name,
            sub.name AS subject_name,
            q.teacher_id,
            t.full_name AS teacher_name,
            COALESCE(q.exam_mode, 'HOMEWORK_QUIZ') AS exam_mode,
            COALESCE(q.assessment_category, 'FORMATIVE') AS assessment_category,
            q.exam_token,
            q.token_expires_at,
            q.start_at,
            q.end_at,
            q.release_at,
            COALESCE(crc.enrolled_count, 0)::bigint AS total_enrolled,
            COALESCE(ats.started_count, 0)::bigint AS started_count,
            COALESCE(ats.in_progress_count, 0)::bigint AS in_progress_count,
            COALESCE(ats.need_grading_count, 0)::bigint AS need_grading_count,
            COALESCE(ats.completed_count, 0)::bigint AS completed_count,
            COALESCE(ats.total_violations, 0)::bigint AS total_violations,
            COALESCE(ats.flagged_students_count, 0)::bigint AS flagged_students_count
        FROM quizzes q
        LEFT JOIN classes c ON c.id = q.class_id
        LEFT JOIN subjects sub ON sub.id = q.subject_id
        LEFT JOIN teachers t ON t.id = q.teacher_id
        LEFT JOIN class_roster_counts crc ON crc.class_id = q.class_id
        LEFT JOIN attempt_summary ats ON ats.quiz_id = q.id
        WHERE q.tenant_id = $1
          AND q.deleted_at IS NULL
          AND (
              $2 = FALSE
              OR q.created_by = $3
              OR q.teacher_id = $3
              OR q.teacher_id IN (
                  SELECT id FROM teachers WHERE (user_id = $3 OR id = $3) AND tenant_id = $1
              )
          )
        ORDER BY COALESCE(q.start_at, q.release_at, q.created_at) DESC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(scoped_teacher_only)
    .bind(actor_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    let now = Utc::now();
    let mut conflict_inputs = Vec::new();
    let mut raw_sessions = Vec::with_capacity(rows.len());

    let mut live_count = 0usize;
    let mut upcoming_count = 0usize;
    let mut ended_count = 0usize;
    let mut draft_count = 0usize;
    let mut flagged_students_total = 0i64;

    for r in rows {
        let id: Uuid = r.get("id");
        let title: String = r.get("title");
        let status: String = r.get("status");
        let is_active: bool = r.get("is_active");
        let questions_count: i32 = r.get("questions_count");
        let duration_minutes: i32 = r.get("duration_minutes");
        let passing_score: i32 = r.get("passing_score");
        let class_id: Option<Uuid> = r.get("class_id");
        let class_name: Option<String> = r.get("class_name");
        let subject_name: Option<String> = r.get("subject_name");
        let teacher_id: Option<Uuid> = r.get("teacher_id");
        let teacher_name: Option<String> = r.get("teacher_name");
        let exam_mode: String = r.get("exam_mode");
        let assessment_category: String = r.get("assessment_category");
        let exam_token: Option<String> = r.try_get("exam_token").ok().flatten();
        let requires_token = exam_token.as_deref().is_some_and(|t| !t.trim().is_empty());
        let token_expires_at: Option<DateTime<Utc>> = r.try_get("token_expires_at").ok().flatten();
        let start_at: Option<DateTime<Utc>> = r.try_get("start_at").ok().flatten();
        let end_at: Option<DateTime<Utc>> = r.try_get("end_at").ok().flatten();
        let release_at: Option<DateTime<Utc>> = r.try_get("release_at").ok().flatten();

        let total_enrolled: i64 = r.get("total_enrolled");
        let started_count: i64 = r.get("started_count");
        let in_progress_count: i64 = r.get("in_progress_count");
        let need_grading_count: i64 = r.get("need_grading_count");
        let completed_count: i64 = r.get("completed_count");
        let total_violations: i64 = r.get("total_violations");
        let flagged_students_count: i64 = r.get("flagged_students_count");

        let eval = evaluate_quiz_schedule(QuizScheduleInput {
            status: &status,
            is_active,
            questions_count,
            start_at,
            end_at,
            release_at,
            student_attempt_status: None,
            student_has_completed: false,
            now,
        });

        match eval.phase {
            ExamPhase::Live => live_count += 1,
            ExamPhase::Upcoming => upcoming_count += 1,
            ExamPhase::Ended => ended_count += 1,
            ExamPhase::Draft => draft_count += 1,
        }
        flagged_students_total += flagged_students_count;

        if matches!(eval.phase, ExamPhase::Live | ExamPhase::Upcoming) {
            if let Some(sched_start) = start_at.or(release_at) {
                conflict_inputs.push(ScheduleConflictInput {
                    quiz_id: id,
                    title: title.clone(),
                    class_id,
                    class_name: class_name.clone(),
                    teacher_id,
                    teacher_name: teacher_name.clone(),
                    start_at: sched_start,
                    end_at,
                    duration_minutes,
                });
            }
        }

        let not_started_count = (total_enrolled - started_count).max(0);
        raw_sessions.push(ProctorSessionOverviewDto {
            id,
            title,
            subject_name,
            class_id,
            class_name,
            teacher_id,
            teacher_name,
            exam_mode,
            assessment_category,
            status,
            exam_phase: eval.phase.as_str().to_string(),
            duration_minutes,
            questions_count,
            passing_score,
            exam_token,
            requires_token,
            token_expires_at,
            start_at,
            end_at,
            release_at,
            seconds_until_start: eval.seconds_until_start,
            seconds_remaining: eval.seconds_remaining,
            total_enrolled,
            started_count,
            in_progress_count,
            need_grading_count,
            completed_count,
            not_started_count,
            total_violations,
            flagged_students_count,
            has_conflict: false,
        });
    }

    let domain_conflicts = detect_proctor_schedule_conflicts(&conflict_inputs);
    let mut conflicting_ids = HashSet::new();
    for c in &domain_conflicts {
        conflicting_ids.insert(c.primary_quiz_id);
        conflicting_ids.insert(c.conflicting_quiz_id);
    }

    for session in &mut raw_sessions {
        session.has_conflict = conflicting_ids.contains(&session.id);
    }

    let conflicts: Vec<ProctorScheduleConflictDto> =
        domain_conflicts.into_iter().map(Into::into).collect();

    let summary = ProctorScheduleSummaryDto {
        total_sessions: raw_sessions.len(),
        live_count,
        upcoming_count,
        ended_count,
        draft_count,
        conflict_count: conflicts.len(),
        flagged_students_total,
    };

    Ok(Json(ApiResponse::success(
        ProctorScheduleDashboardResponse {
            server_now: now,
            summary,
            conflicts,
            sessions: raw_sessions,
        },
        req_ctx.request_id,
    )))
}

/// Returns the live exam room monitor for a specific quiz, joining the class's active student
/// roster with quiz attempts, answered counts, heartbeat online status, and violation events.
pub async fn get_proctor_monitor(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ProctorMonitorResponse>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizRead)?;
    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    ensure_proctoring_schema(&ctx.pool).await;

    let meta = sqlx::query(
        r#"
        SELECT q.id, q.tenant_id, q.lesson_id, q.title, q.description, q.time_limit_minutes,
               q.passing_score, q.max_score, q.max_attempts, q.shuffle_questions, q.shuffle_choices,
               q.start_at, q.end_at, q.status, q.questions_count, q.is_active,
               q.created_at, q.updated_at, q.class_id, q.teacher_id, q.created_by, q.session_id,
               q.exam_mode, q.assessment_category, q.exam_token, q.token_expires_at,
               q.max_token_attempts, q.release_at,
               c.name AS class_name, sub.name AS subject_name, t.full_name AS teacher_name
        FROM quizzes q
        LEFT JOIN classes c ON c.id = q.class_id
        LEFT JOIN subjects sub ON sub.id = q.subject_id
        LEFT JOIN teachers t ON t.id = q.teacher_id
        WHERE q.id = $1 AND q.tenant_id = $2 AND q.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Quiz {id} not found"), &req_ctx.request_id))?;

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

    let now = Utc::now();
    let dur = meta
        .get::<Option<i32>, _>("time_limit_minutes")
        .unwrap_or(30);
    let raw_token: Option<String> = meta.try_get("exam_token").ok().flatten();
    let req_tok = raw_token.as_deref().is_some_and(|t| !t.trim().is_empty());
    let quiz_questions_count: i32 = meta.get("questions_count");
    let quiz_max_score: i32 = meta.get("max_score");
    let quiz_passing_score: i32 = meta.get("passing_score");

    let mut quiz_resp = QuizResponse {
        id: meta.get("id"),
        tenant_id: meta_tenant_id,
        lesson_id: meta.get("lesson_id"),
        title: meta.get("title"),
        description: meta.get("description"),
        duration_minutes: dur,
        time_limit_minutes: dur,
        passing_score: quiz_passing_score,
        max_score: quiz_max_score,
        max_attempts: meta.get("max_attempts"),
        shuffle_questions: meta.get("shuffle_questions"),
        shuffle_choices: meta.get("shuffle_choices"),
        start_at: meta.try_get("start_at").ok().flatten(),
        end_at: meta.try_get("end_at").ok().flatten(),
        status: meta.get("status"),
        questions_count: quiz_questions_count,
        is_active: meta.get("is_active"),
        created_at: meta.get("created_at"),
        updated_at: meta.get("updated_at"),
        class_id: meta_class_id,
        class_name: meta.try_get("class_name").ok().flatten(),
        subject_name: meta.try_get("subject_name").ok().flatten(),
        teacher_id: meta_teacher_id,
        teacher_name: meta.try_get("teacher_name").ok().flatten(),
        session_id: meta.try_get("session_id").ok().flatten(),
        release_at: meta.try_get("release_at").ok().flatten(),
        exam_mode: meta
            .try_get::<Option<String>, _>("exam_mode")
            .ok()
            .flatten()
            .or_else(|| Some("HOMEWORK_QUIZ".to_string())),
        assessment_category: meta
            .try_get::<Option<String>, _>("assessment_category")
            .ok()
            .flatten()
            .or_else(|| Some("FORMATIVE".to_string())),
        exam_token: raw_token,
        requires_token: req_tok,
        token_expires_at: meta.try_get("token_expires_at").ok().flatten(),
        max_token_attempts: meta
            .try_get::<Option<i32>, _>("max_token_attempts")
            .ok()
            .flatten()
            .or(Some(5)),
        student_attempt_status: None,
        student_attempts_count: None,
        student_has_completed: None,
        student_last_score: None,
        student_last_attempt_id: None,
        exam_phase: "DRAFT".to_string(),
        is_open_for_student: false,
        seconds_until_start: None,
        seconds_remaining: None,
    };
    quiz_resp.apply_schedule_evaluation(now);

    let rows = sqlx::query(
        r#"
        WITH roster AS (
            SELECT DISTINCT ON (s.id)
                s.id AS student_id,
                s.user_id,
                COALESCE(NULLIF(s.full_name, ''), u.full_name, 'Siswa') AS student_name,
                COALESCE(NULLIF(s.nisn, ''), u.username, '-') AS student_nisn,
                c.name AS class_name
            FROM enrollments en
            JOIN students s ON s.id = en.student_id
            LEFT JOIN users u ON u.id = s.user_id
            LEFT JOIN classes c ON c.id = en.class_id
            WHERE $3::uuid IS NOT NULL
              AND en.class_id = $3
              AND s.tenant_id = $2
              AND s.deleted_at IS NULL
              AND (en.status ILIKE 'active' OR en.status IS NULL)
            ORDER BY s.id, s.full_name ASC
        ),
        latest_attempts AS (
            SELECT DISTINCT ON (COALESCE(s.id, qa.student_id))
                COALESCE(s.id, qa.student_id) AS canonical_student_id,
                s.user_id AS student_user_id,
                COALESCE(NULLIF(s.full_name, ''), u.full_name, 'Siswa') AS att_student_name,
                COALESCE(NULLIF(s.nisn, ''), u.username, '-') AS att_student_nisn,
                qa.id AS attempt_id,
                qa.started_at,
                qa.completed_at,
                qa.score,
                qa.total_points,
                qa.status AS attempt_status,
                COALESCE(qa.leave_violations_count, 0) AS leave_violations_count,
                qa.last_violation_at,
                qa.last_heartbeat_at,
                qa.last_device_platform,
                (SELECT COUNT(*)::bigint FROM attempt_answers aa WHERE aa.attempt_id = qa.id) AS answered_count
            FROM quiz_attempts qa
            LEFT JOIN students s
                ON s.tenant_id = qa.tenant_id
               AND (s.id = qa.student_id OR s.user_id = qa.student_id)
            LEFT JOIN users u ON u.id = qa.student_id OR u.id = s.user_id
            WHERE qa.quiz_id = $1 AND qa.tenant_id = $2
            ORDER BY COALESCE(s.id, qa.student_id), qa.created_at DESC
        )
        SELECT
            COALESCE(r.student_id, la.canonical_student_id) AS student_id,
            COALESCE(r.user_id, la.student_user_id) AS user_id,
            COALESCE(r.student_name, la.att_student_name, 'Siswa') AS student_name,
            COALESCE(r.student_nisn, la.att_student_nisn, '-') AS student_nisn,
            r.class_name,
            la.attempt_id,
            la.started_at,
            la.completed_at,
            la.score,
            COALESCE(la.total_points, $4) AS total_points,
            la.attempt_status,
            COALESCE(la.leave_violations_count, 0) AS leave_violations_count,
            la.last_violation_at,
            la.last_heartbeat_at,
            la.last_device_platform,
            COALESCE(la.answered_count, 0)::bigint AS answered_count
        FROM roster r
        FULL OUTER JOIN latest_attempts la ON la.canonical_student_id = r.student_id
        ORDER BY
            CASE
                WHEN COALESCE(la.leave_violations_count, 0) > 0 THEN 0
                WHEN LOWER(COALESCE(la.attempt_status, '')) IN ('in_progress', 'started', 'created') THEN 1
                WHEN la.attempt_id IS NULL THEN 2
                WHEN LOWER(COALESCE(la.attempt_status, '')) IN ('graded', 'reviewed') THEN 4
                ELSE 3
            END ASC,
            student_name ASC
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .bind(meta_class_id)
    .bind(quiz_max_score)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    let mut not_started_count = 0usize;
    let mut in_progress_count = 0usize;
    let mut need_grading_count = 0usize;
    let mut completed_count = 0usize;
    let mut flagged_count = 0usize;
    let mut total_violations = 0i64;
    let mut participants = Vec::with_capacity(rows.len());

    for r in rows {
        let attempt_id: Option<Uuid> = r.try_get("attempt_id").ok().flatten();
        let attempt_status: Option<String> = r.try_get("attempt_status").ok().flatten();
        let proctor_status = classify_proctor_participant_status(attempt_status.as_deref());

        match proctor_status {
            ProctorParticipantStatus::NotStarted => not_started_count += 1,
            ProctorParticipantStatus::InProgress => in_progress_count += 1,
            ProctorParticipantStatus::NeedGrading => need_grading_count += 1,
            ProctorParticipantStatus::Completed => completed_count += 1,
        }

        let leave_violations_count: i32 = r
            .try_get::<Option<i32>, _>("leave_violations_count")
            .ok()
            .flatten()
            .unwrap_or(0);
        if leave_violations_count > 0 {
            flagged_count += 1;
        }
        total_violations += i64::from(leave_violations_count);

        let started_at: Option<DateTime<Utc>> = r.try_get("started_at").ok().flatten();
        let completed_at: Option<DateTime<Utc>> = r.try_get("completed_at").ok().flatten();
        let last_heartbeat_at: Option<DateTime<Utc>> =
            r.try_get("last_heartbeat_at").ok().flatten();

        let elapsed_seconds = started_at.map(|st| {
            let end_ref = completed_at.unwrap_or(now);
            (end_ref - st).num_seconds().max(0)
        });

        let is_online = proctor_status == ProctorParticipantStatus::InProgress
            && last_heartbeat_at
                .or(started_at)
                .is_some_and(|ts| (now - ts).num_seconds() <= 60);

        let raw_score: Option<i32> = r.try_get("score").ok().flatten();
        let att_total_pts: i32 = r.try_get("total_points").unwrap_or(quiz_max_score);
        let eff_max = if att_total_pts > 0 {
            att_total_pts
        } else if quiz_max_score > 0 {
            quiz_max_score
        } else {
            100
        };

        let percentage = raw_score.map(|sc| ((sc * 100) / eff_max.max(1)).clamp(0, 100));
        let passed = if matches!(
            proctor_status,
            ProctorParticipantStatus::Completed | ProctorParticipantStatus::NeedGrading
        ) {
            percentage.map(|p| p >= quiz_passing_score)
        } else {
            None
        };

        participants.push(ProctorParticipantRowDto {
            student_id: r.get("student_id"),
            user_id: r.try_get("user_id").ok().flatten(),
            student_name: r.get("student_name"),
            student_nisn: r.get("student_nisn"),
            class_name: r.try_get("class_name").ok().flatten(),
            attempt_id,
            raw_attempt_status: attempt_status,
            proctor_status: proctor_status.as_str().to_string(),
            started_at,
            completed_at,
            elapsed_seconds,
            answered_count: r.try_get("answered_count").unwrap_or(0),
            total_questions: quiz_questions_count,
            score: raw_score,
            total_points: eff_max,
            percentage,
            passed,
            leave_violations_count,
            last_violation_at: r.try_get("last_violation_at").ok().flatten(),
            last_heartbeat_at,
            is_online,
            last_device_platform: r.try_get("last_device_platform").ok().flatten(),
        });
    }

    let event_rows = sqlx::query(
        r#"
        SELECT
            e.id,
            e.student_id,
            COALESCE(NULLIF(s.full_name, ''), u.full_name, 'Siswa') AS student_name,
            e.attempt_id,
            e.event_type,
            e.detail,
            e.device_platform,
            e.created_at
        FROM quiz_proctor_events e
        LEFT JOIN students s ON s.id = e.student_id
        LEFT JOIN users u ON u.id = s.user_id OR u.id = e.student_id
        WHERE e.quiz_id = $1 AND e.tenant_id = $2
        ORDER BY e.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let recent_events: Vec<ProctorEventDto> = event_rows
        .into_iter()
        .map(|er| ProctorEventDto {
            id: er.get("id"),
            student_id: er.get("student_id"),
            student_name: er.get("student_name"),
            attempt_id: er.get("attempt_id"),
            event_type: er.get("event_type"),
            detail: er.try_get("detail").ok().flatten(),
            device_platform: er.try_get("device_platform").ok().flatten(),
            created_at: er.get("created_at"),
        })
        .collect();

    let summary = ProctorMonitorSummaryDto {
        total_enrolled: participants.len(),
        not_started_count,
        in_progress_count,
        need_grading_count,
        completed_count,
        flagged_count,
        total_violations,
    };

    Ok(Json(ApiResponse::success(
        ProctorMonitorResponse {
            server_now: now,
            quiz: quiz_resp,
            summary,
            participants,
            recent_events,
        },
        req_ctx.request_id,
    )))
}

/// Rotates or sets a proctored CBT token without kicking already-started students.
pub async fn rotate_token(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    payload: Option<Json<RotateTokenRequest>>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizUpdate)?;

    let meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, end_at FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Quiz {id} not found"), &req_ctx.request_id))?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta.get("tenant_id"),
        meta.get("class_id"),
        meta.get("teacher_id"),
        meta.get("created_by"),
    )
    .await?;

    let req_body = payload.map(|Json(p)| p);
    let custom = req_body.as_ref().and_then(|b| b.custom_token.as_deref());
    let new_token = generate_ambiguity_free_exam_token(custom)
        .map_err(|m| validation_err(m, &req_ctx.request_id))?;

    let expires_at = req_body
        .and_then(|b| b.expires_in_minutes)
        .filter(|m| *m > 0)
        .map(|mins| Utc::now() + chrono::Duration::minutes(i64::from(mins.min(1440))));

    sqlx::query(
        r#"
        UPDATE quizzes
        SET exam_mode = 'PROCTORED_CBT',
            exam_token = $1,
            token_expires_at = COALESCE($2, end_at, token_expires_at),
            updated_at = NOW()
        WHERE id = $3 AND tenant_id = $4 AND deleted_at IS NULL
        "#,
    )
    .bind(&new_token)
    .bind(expires_at)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    get_by_id(State(ctx), req_ctx, Path(id)).await
}

/// Extends an active or recently ended CBT exam session window (`end_at`).
pub async fn extend_time(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExtendTimeRequest>,
) -> Result<Json<ApiResponse<QuizResponse>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizUpdate)?;

    let meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, end_at FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Quiz {id} not found"), &req_ctx.request_id))?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        meta.get("tenant_id"),
        meta.get("class_id"),
        meta.get("teacher_id"),
        meta.get("created_by"),
    )
    .await?;

    let current_end: Option<DateTime<Utc>> = meta.try_get("end_at").ok().flatten();
    let next_end = compute_extended_end_at(current_end, payload.extra_minutes, Utc::now())
        .map_err(|m| validation_err(m, &req_ctx.request_id))?;

    sqlx::query(
        r#"
        UPDATE quizzes
        SET end_at = $1,
            token_expires_at = CASE WHEN token_expires_at IS NOT NULL THEN $1 ELSE NULL END,
            updated_at = NOW()
        WHERE id = $2 AND tenant_id = $3 AND deleted_at IS NULL
        "#,
    )
    .bind(next_end)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    get_by_id(State(ctx), req_ctx, Path(id)).await
}

/// Forces submission and objective scoring of a student's in-progress attempt by a proctor.
pub async fn force_submit_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((quiz_id, attempt_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizUpdate)?;
    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    ensure_quiz_variatif_schema(&ctx.pool).await;
    ensure_proctoring_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, title, passing_score, max_score FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Quiz {quiz_id} not found"), &req_ctx.request_id))?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let att_row = sqlx::query(
        r#"
        SELECT qa.id, qa.student_id, qa.total_points, s.user_id
        FROM quiz_attempts qa
        LEFT JOIN students s ON s.id = qa.student_id OR s.user_id = qa.student_id
        WHERE qa.id = $1 AND qa.quiz_id = $2 AND qa.tenant_id = $3
        "#,
    )
    .bind(attempt_id)
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| {
        not_found_err(
            &format!("Attempt {attempt_id} not found"),
            &req_ctx.request_id,
        )
    })?;

    let student_id: Uuid = att_row.get("student_id");
    let total_points: i32 = att_row
        .try_get::<Option<i32>, _>("total_points")
        .ok()
        .flatten()
        .unwrap_or(0);
    let user_id: Option<Uuid> = att_row.try_get("user_id").ok();

    score_single_choice_pg_answers(&ctx.pool, attempt_id).await;
    score_variatif_objective_answers(&ctx.pool, attempt_id).await;

    let new_score: i32 = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(SUM(points_earned), 0)::int FROM attempt_answers WHERE attempt_id = $1",
    )
    .bind(attempt_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(0);

    let has_essay: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM quiz_questions WHERE quiz_id = $1 AND UPPER(COALESCE(question_type, '')) IN ('ESSAY', 'URAIAN'))",
    )
    .bind(quiz_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(false);

    let qtitle: String = quiz_meta.get("title");
    let passing_score: i32 = quiz_meta.try_get("passing_score").unwrap_or(75);
    let max_score: i32 = quiz_meta.try_get("max_score").unwrap_or(100);

    let eff_max = if total_points > 0 {
        total_points
    } else if max_score > 0 {
        max_score
    } else {
        100
    };
    let percentage = ((new_score * 100) / eff_max.max(1)).clamp(0, 100);
    let is_passed = percentage >= passing_score;
    let next_status = if has_essay { "submitted" } else { "graded" };

    sqlx::query(
        r#"
        UPDATE quiz_attempts
        SET score = $1,
            passed = $2,
            status = $3,
            completed_at = COALESCE(completed_at, NOW()),
            updated_at = NOW()
        WHERE id = $4 AND tenant_id = $5
        "#,
    )
    .bind(new_score)
    .bind(if has_essay { false } else { is_passed })
    .bind(next_status)
    .bind(attempt_id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    let _ = sqlx::query(
        r#"
        INSERT INTO quiz_proctor_events (id, tenant_id, quiz_id, attempt_id, student_id, event_type, detail, device_platform)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, 'PROCTOR_FORCE_SUBMIT', 'Sesi diselesaikan paksa oleh proktor/pengawas ujian', 'PROCTOR_WEB')
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(quiz_id)
    .bind(attempt_id)
    .bind(student_id)
    .execute(&ctx.pool)
    .await;

    if !has_essay {
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
            let notif_title = format!("Hasil CBT: {}", qtitle);
            let notif_body = format!(
                "Ujian CBT ditutup pengawas. Skor objektif: {} / 100.",
                percentage
            );
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
        }

        crate::presentation::learning::grade_sync::sync_quiz_attempt_to_gradebook(
            &ctx.pool,
            req_ctx.tenant_id,
            attempt_id,
        )
        .await;
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "attempt_id": attempt_id,
            "status": next_status,
            "score": new_score,
            "percentage": percentage,
        }),
        req_ctx.request_id,
    )))
}

/// Reopens a student attempt back to `in_progress` when an unexpected device crash occurs.
pub async fn reopen_attempt(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((quiz_id, attempt_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    require_staff_permission(&req_ctx, Permission::LearningQuizUpdate)?;
    ensure_proctoring_schema(&ctx.pool).await;

    let quiz_meta = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM quizzes WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Quiz {quiz_id} not found"), &req_ctx.request_id))?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        quiz_meta.get("tenant_id"),
        quiz_meta.get("class_id"),
        quiz_meta.get("teacher_id"),
        quiz_meta.get("created_by"),
    )
    .await?;

    let att_row = sqlx::query(
        "SELECT student_id FROM quiz_attempts WHERE id = $1 AND quiz_id = $2 AND tenant_id = $3",
    )
    .bind(attempt_id)
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| {
        not_found_err(
            &format!("Attempt {attempt_id} not found"),
            &req_ctx.request_id,
        )
    })?;

    let student_id: Uuid = att_row.get("student_id");

    sqlx::query(
        r#"
        UPDATE quiz_attempts
        SET status = 'in_progress',
            completed_at = NULL,
            updated_at = NOW()
        WHERE id = $1 AND quiz_id = $2 AND tenant_id = $3
        "#,
    )
    .bind(attempt_id)
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?;

    let _ = sqlx::query(
        r#"
        INSERT INTO quiz_proctor_events (id, tenant_id, quiz_id, attempt_id, student_id, event_type, detail, device_platform)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, 'PROCTOR_REOPEN_ATTEMPT', 'Proktor membuka kembali sesi ujian siswa untuk dilanjutkan', 'PROCTOR_WEB')
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(quiz_id)
    .bind(attempt_id)
    .bind(student_id)
    .execute(&ctx.pool)
    .await;

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "attempt_id": attempt_id,
            "status": "in_progress"
        }),
        req_ctx.request_id,
    )))
}

/// Records client proctoring heartbeat or focus-loss violation events for an active exam attempt.
pub async fn record_proctor_event(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((quiz_id, attempt_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<RecordProctorEventRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    crate::middleware::require_permission(&req_ctx.actor, Permission::LearningQuizRead)
        .map_err(|_| forbidden_err("Insufficient permissions", &req_ctx.request_id))?;

    ensure_proctoring_schema(&ctx.pool).await;

    let att_row = sqlx::query(
        r#"
        SELECT qa.student_id, qa.status, COALESCE(qa.leave_violations_count, 0) AS leave_violations_count,
               s.user_id AS student_user_id
        FROM quiz_attempts qa
        LEFT JOIN students s ON s.id = qa.student_id
        WHERE qa.id = $1 AND qa.quiz_id = $2 AND qa.tenant_id = $3
        "#,
    )
    .bind(attempt_id)
    .bind(quiz_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| db_err(e, &req_ctx.request_id))?
    .ok_or_else(|| not_found_err(&format!("Attempt {attempt_id} not found"), &req_ctx.request_id))?;

    let att_student_id: Uuid = att_row.get("student_id");
    let att_status: String = att_row.get("status");
    let existing_violations: i32 = att_row.try_get("leave_violations_count").unwrap_or(0);
    let student_user_id: Option<Uuid> = att_row.try_get("student_user_id").ok().flatten();

    if !actor_is_teacher_or_admin(&req_ctx) {
        let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
        let resolved_sid = crate::authorization_helpers::AuthorizationScope::resolve_student_id(
            &ctx.pool,
            req_ctx.tenant_id,
            actor_id,
        )
        .await
        .ok()
        .flatten();

        let is_owner = att_student_id == actor_id
            || student_user_id == Some(actor_id)
            || resolved_sid == Some(att_student_id);
        if !is_owner {
            return Err(forbidden_err(
                "Tidak diizinkan mencatat event proktor untuk sesi siswa lain.",
                &req_ctx.request_id,
            ));
        }
    }

    let att_status_lower = att_status.trim().to_lowercase();
    if !matches!(
        att_status_lower.as_str(),
        "in_progress" | "started" | "created"
    ) {
        return Ok(Json(ApiResponse::success(
            serde_json::json!({
                "recorded": false,
                "leave_violations_count": existing_violations,
                "reason": "attempt_not_active"
            }),
            req_ctx.request_id,
        )));
    }

    let event_type: String = payload
        .event_type
        .trim()
        .to_uppercase()
        .chars()
        .take(40)
        .collect();
    let platform: Option<String> = payload
        .device_platform
        .as_deref()
        .map(|s| s.trim().to_uppercase().chars().take(32).collect::<String>())
        .filter(|s| !s.is_empty());

    let is_violation = is_violation_event_type(&event_type);

    let updated_violations = if is_violation {
        let count: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE quiz_attempts
            SET leave_violations_count = COALESCE(leave_violations_count, 0) + 1,
                last_violation_at = NOW(),
                last_heartbeat_at = NOW(),
                last_device_platform = COALESCE($1, last_device_platform)
            WHERE id = $2 AND tenant_id = $3
            RETURNING leave_violations_count
            "#,
        )
        .bind(&platform)
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(existing_violations + 1);

        let _ = sqlx::query(
            r#"
            INSERT INTO quiz_proctor_events (id, tenant_id, quiz_id, attempt_id, student_id, event_type, detail, device_platform)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(quiz_id)
        .bind(attempt_id)
        .bind(att_student_id)
        .bind(&event_type)
        .bind(payload.detail.as_deref())
        .bind(platform.as_deref())
        .execute(&ctx.pool)
        .await;

        count
    } else {
        let _ = sqlx::query(
            r#"
            UPDATE quiz_attempts
            SET last_heartbeat_at = NOW(),
                last_device_platform = COALESCE($1, last_device_platform)
            WHERE id = $2 AND tenant_id = $3
            "#,
        )
        .bind(&platform)
        .bind(attempt_id)
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await;

        existing_violations
    };

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "recorded": true,
            "event_type": event_type,
            "is_violation": is_violation,
            "leave_violations_count": updated_violations,
        }),
        req_ctx.request_id,
    )))
}
