use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize, Default)]
pub struct AssignmentFilterQuery {
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub date: Option<String>,
    pub session_id: Option<Uuid>,
}

use super::interactive_game::{
    calculate_total_percentage_score, compile_crossword_interaction_config,
    grade_interactive_answer, is_interactive_game_type, shuffle_interaction_for_student,
};
use super::dto::{
    assignment_question_dto::{
        AssignmentChoiceDto, AssignmentQuestionDto, SubmissionAnswerDetailDto,
    },
    assignment_response::AssignmentResponse,
    create_assignment_request::CreateAssignmentRequest,
    grade_submission_request::GradeSubmissionRequest,
    submission_response::SubmissionResponse,
    submit_assignment_request::SubmitAssignmentRequest,
    update_assignment_request::UpdateAssignmentRequest,
};

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::assignment::{
    archive_assignment::ArchiveAssignmentCommand, close_assignment::CloseAssignmentCommand,
    create_assignment::CreateAssignmentCommand, delete_assignment::DeleteAssignmentCommand,
    grade_submission::GradeSubmissionCommand, publish_assignment::PublishAssignmentCommand,
    submit_assignment::SubmitAssignmentCommand, update_assignment::UpdateAssignmentCommand,
};

pub fn assignment_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route(
            "/upload",
            post(upload_submission_file).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/publish", post(publish))
        .route("/{id}/close", post(close))
        .route("/{id}/archive", post(archive))
        .route("/{id}/submit", post(submit))
        .route("/{id}/submissions", get(get_submissions))
        .route("/{id}/submissions/{submission_id}/grade", post(grade))
}

static INTERACTION_CONFIG_READY: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn ensure_interaction_config_column(_pool: &sqlx::PgPool) {
    // Schema managed exclusively via SQLx migrations (20261125020000_add_multitenant_hot_indexes_and_analytics_projection.sql).
    INTERACTION_CONFIG_READY.get_or_init(|| async {}).await;
}

async fn persist_assignment_questions(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
    questions: Vec<AssignmentQuestionDto>,
) -> Vec<AssignmentQuestionDto> {
    ensure_interaction_config_column(pool).await;
    let mut created_questions = Vec::with_capacity(questions.len());
    for (q_idx, q) in questions.into_iter().enumerate() {
        let question_id = q.id.unwrap_or_else(Uuid::now_v7);
        let order_idx = q.order_index.unwrap_or(q_idx as i32);
        let points = q.points.unwrap_or(10);
        let normalized_interaction_config = if q.question_type.trim().eq_ignore_ascii_case("CROSSWORD") {
            q.interaction_config
                .as_ref()
                .map(compile_crossword_interaction_config)
        } else {
            q.interaction_config
        };
        let _ = sqlx::query(
            r#"
            INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index, interaction_config)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(question_id)
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(&q.question_text)
        .bind(&q.question_type)
        .bind(points)
        .bind(order_idx)
        .bind(&normalized_interaction_config)
        .execute(pool)
        .await;

        let mut created_choices = Vec::with_capacity(q.choices.len());
        for (c_idx, c) in q.choices.into_iter().enumerate() {
            let choice_id = c.id.unwrap_or_else(Uuid::now_v7);
            let c_order_idx = c.order_index.unwrap_or(c_idx as i32);
            let is_correct = c.is_correct.unwrap_or(false);
            let _ = sqlx::query(
                r#"
                INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(choice_id)
            .bind(question_id)
            .bind(&c.choice_text)
            .bind(is_correct)
            .bind(c_order_idx)
            .execute(pool)
            .await;

            created_choices.push(AssignmentChoiceDto {
                id: Some(choice_id),
                choice_text: c.choice_text,
                is_correct: Some(is_correct),
                order_index: Some(c_order_idx),
            });
        }

        created_questions.push(AssignmentQuestionDto {
            id: Some(question_id),
            question_text: q.question_text,
            question_type: q.question_type,
            points: Some(points),
            order_index: Some(order_idx),
            choices: created_choices,
            interaction_config: normalized_interaction_config,
        });
    }
    created_questions
}

async fn fetch_assignment_questions(
    pool: &sqlx::PgPool,
    assignment_id: Uuid,
    hide_correct_answers: bool,
) -> Result<Vec<AssignmentQuestionDto>, sqlx::Error> {
    ensure_interaction_config_column(pool).await;
    let q_rows = sqlx::query(
        r#"
        SELECT id, question_text, question_type, points, order_index, interaction_config
        FROM assignment_questions
        WHERE assignment_id = $1
        ORDER BY order_index ASC, created_at ASC
        "#,
    )
    .bind(assignment_id)
    .fetch_all(pool)
    .await?;

    if q_rows.is_empty() {
        return Ok(Vec::new());
    }

    let q_ids: Vec<Uuid> = q_rows.iter().map(|r| r.get::<Uuid, _>("id")).collect();
    let c_rows = sqlx::query(
        r#"
        SELECT id, question_id, choice_text, is_correct, order_index
        FROM assignment_question_choices
        WHERE question_id = ANY($1)
        ORDER BY order_index ASC, created_at ASC
        "#,
    )
    .bind(&q_ids)
    .fetch_all(pool)
    .await?;

    let mut questions = Vec::new();
    for q in q_rows {
        let q_id: Uuid = q.get("id");
        let q_type: String = q.get("question_type");
        let raw_cfg: Option<serde_json::Value> = q.try_get("interaction_config").ok().flatten();
        let effective_cfg = if hide_correct_answers && is_interactive_game_type(&q_type) {
            raw_cfg.map(|cfg| shuffle_interaction_for_student(&q_type, &cfg))
        } else {
            raw_cfg
        };

        let choices = c_rows
            .iter()
            .filter(|c| c.get::<Uuid, _>("question_id") == q_id)
            .map(|c| AssignmentChoiceDto {
                id: Some(c.get("id")),
                choice_text: c.get("choice_text"),
                is_correct: if hide_correct_answers {
                    None
                } else {
                    Some(c.get("is_correct"))
                },
                order_index: Some(c.get("order_index")),
            })
            .collect();

        questions.push(AssignmentQuestionDto {
            id: Some(q_id),
            question_text: q.get("question_text"),
            question_type: q_type,
            points: Some(q.get("points")),
            order_index: Some(q.get("order_index")),
            choices,
            interaction_config: effective_cfg,
        });
    }

    Ok(questions)
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        title: payload.title,
        description: payload.description.clone(),
        instructions: payload.instructions,
        max_score: payload.max_score.unwrap_or(100),
        due_at: payload.due_at,
        assignment_type: payload.assignment_type,
    };

    let assignment = ctx
        .create_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // ── FCM + in-app untuk TUGAS baru (agar masuk walau HP idle) ──
    let assignment_id_for_notif = assignment.id;
    let assignment_title_for_notif = assignment.title.clone();

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let teacher_id = if let Some(aid) = actor_id {
        sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM teachers WHERE (user_id = $1 OR id = $1) AND tenant_id = $2 LIMIT 1"#
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

    // Resolve class_id from UUID or class name string (e.g. "PAKET C10" or "PAKET C 10")
    let target_class_id: Option<Uuid> = match payload.class_id {
        Some(ref cid_str) if !cid_str.trim().is_empty() => {
            if let Ok(u) = Uuid::parse_str(cid_str.trim()) {
                Some(u)
            } else {
                sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2 OR REPLACE(name, ' ', '') ILIKE REPLACE($2, ' ', '')) LIMIT 1"#,
                )
                .bind(req_ctx.tenant_id)
                .bind(cid_str.trim())
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
        _ => {
            if let Some(ref desc) = payload.description {
                let second_part = desc.split('•').nth(1).map(|s| s.trim()).unwrap_or("");
                if !second_part.is_empty() && !second_part.eq_ignore_ascii_case("Semua Rombel") {
                    sqlx::query_scalar::<_, Uuid>(
                        r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2 OR REPLACE(name, ' ', '') ILIKE REPLACE($2, ' ', '')) LIMIT 1"#,
                    )
                    .bind(req_ctx.tenant_id)
                    .bind(second_part)
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

    // Resolve subject_id: priority payload.subject_id, fallback to first part of description
    let subject_id: Option<Uuid> = if let Some(sid) = payload.subject_id {
        Some(sid)
    } else if let Some(ref desc) = payload.description {
        let first_part = desc.split('•').next().map(|s| s.trim()).unwrap_or("");
        if !first_part.is_empty() {
            sqlx::query_scalar::<_, Uuid>(
                r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
            )
            .bind(req_ctx.tenant_id)
            .bind(first_part)
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

    let final_teacher_id: Option<Uuid> = if let Some(tid) = payload.teacher_id {
        Some(tid)
    } else if teacher_id.is_some() {
        teacher_id
    } else if let Some(aid) = actor_id {
        let t_id = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"
            SELECT (SELECT t.id FROM teachers t WHERE (t.user_id = $1 OR t.id = $1) AND t.tenant_id = $2 LIMIT 1) as "teacher_id"
            "#,
        )
        .bind(aid)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .flatten();

        if t_id.is_some() {
            t_id
        } else if let Some(ref desc) = payload.description {
            let third_part = desc.split('•').nth(2).map(|s| s.trim()).unwrap_or("");
            if !third_part.is_empty() {
                sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT id FROM teachers WHERE tenant_id = $1 AND (full_name ILIKE $2 OR full_name = $2) LIMIT 1"#,
                )
                .bind(req_ctx.tenant_id)
                .bind(third_part)
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
    } else {
        None
    };

    let is_scheduled = payload.release_at.is_some_and(|r| r > chrono::Utc::now());
    let initial_status = match payload.status.as_deref() {
        Some("draft") => "draft".to_string(),
        Some("scheduled") => "scheduled".to_string(),
        Some("archived") => "archived".to_string(),
        Some("published") => "published".to_string(),
        _ => if is_scheduled { "scheduled".to_string() } else { "published".to_string() },
    };
    let is_live_published = initial_status == "published";
    let is_archived = initial_status == "archived";
    let notification_dispatched = is_live_published;

    let final_release_at = if is_live_published {
        payload.release_at.or_else(|| Some(chrono::Utc::now()))
    } else {
        payload.release_at
    };

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
    let assessment_category = crate::presentation::learning::grade_sync::normalize_assessment_category(
        payload.assessment_category.as_deref(),
        Some(&assignment_title_for_notif),
    );

    if let Err(e) = sqlx::query(
        r#"UPDATE assignments 
           SET class_id = $1, subject_id = $2, teacher_id = $3, created_by = $4, 
               status = $5, release_at = $6, notification_dispatched = $7, is_active = $8,
               assessment_category = $10
           WHERE id = $9"#,
    )
    .bind(target_class_id)
    .bind(subject_id)
    .bind(final_teacher_id)
    .bind(actor_id)
    .bind(&initial_status)
    .bind(final_release_at)
    .bind(notification_dispatched)
    .bind(!is_archived)
    .bind(assignment.id)
    .bind(&assessment_category)
    .execute(&ctx.pool)
    .await {
        tracing::error!("Failed to update assignment metadata: {:?}", e);
    }

    // Auto-archive previous active assignments for this class & subject if requested or if publishing live
    if payload.replace_previous.unwrap_or(true) && is_live_published {
        if let (Some(cid), Some(sid)) = (target_class_id, subject_id) {
            let _ = sqlx::query(
                r#"
                UPDATE assignments 
                SET status = 'archived', is_active = false, updated_at = NOW() 
                WHERE tenant_id = $1 
                  AND class_id = $2 
                  AND subject_id = $3 
                  AND id != $4 
                  AND is_active = true
                "#
            )
            .bind(req_ctx.tenant_id)
            .bind(cid)
            .bind(sid)
            .bind(assignment.id)
            .execute(&ctx.pool)
            .await;
        }
    }

    let (class_name, subject_name, teacher_name) = tokio::join!(
        async {
            if let Some(cid) = target_class_id {
                sqlx::query_scalar::<_, String>(r#"SELECT name FROM classes WHERE id = $1"#)
                    .bind(cid)
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        },
        async {
            if let Some(sid) = subject_id {
                sqlx::query_scalar::<_, String>(r#"SELECT name FROM subjects WHERE id = $1"#)
                    .bind(sid)
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        },
        async {
            if let Some(tid) = final_teacher_id {
                sqlx::query_scalar::<_, String>(r#"SELECT full_name FROM teachers WHERE id = $1"#)
                    .bind(tid)
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        },
    );

    let questions_to_return = if let Some(questions) = payload.questions {
        persist_assignment_questions(&ctx.pool, req_ctx.tenant_id, assignment.id, questions).await
    } else {
        Vec::new()
    };

    let mut resp = AssignmentResponse::from(assignment);
    resp.status = initial_status;
    resp.release_at = payload.release_at.or_else(|| Some(chrono::Utc::now()));
    resp.class_id = target_class_id;
    resp.class_name = class_name;
    resp.subject_name = subject_name;
    resp.teacher_name = teacher_name;
    resp.assessment_category = Some(assessment_category);
    resp.has_game_questions = questions_to_return
        .iter()
        .any(|q| is_interactive_game_type(&q.question_type));
    resp.questions = questions_to_return;

    // Kirim push TUGAS baru ke siswa rombel target (atau semua siswa bila tanpa kelas)
    // HANYA JIKA status PUBLISHED (bukan DRAFT, bukan SCHEDULED, bukan ARCHIVED).
    if is_live_published {
        let t = format!("📝 Tugas Baru: {}", assignment_title_for_notif);
        let b = match (&resp.class_name, &resp.teacher_name) {
            (Some(c), Some(g)) => format!(
                "{} memberi tugas baru untuk kelas {}. Kerjakan sebelum tenggat!",
                g, c
            ),
            (Some(c), None) => format!(
                "Ada tugas baru untuk kelas {}. Kerjakan sebelum tenggat!",
                c
            ),
            _ => format!(
                "Ada tugas baru: {}. Kerjakan sebelum tenggat!",
                assignment_title_for_notif
            ),
        };
        let tid = req_ctx.tenant_id;
        let cid = target_class_id;
        let title_c = t.clone();
        let body_c = b.clone();
        let _ = sqlx::query(
            r#"
            INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
            SELECT
                gen_random_uuid(), s.tenant_id, s.user_id, $1, $2,
                'ASSIGNMENT', 'in_app', 'assignment', $4, FALSE, NOW()
            FROM students s
            JOIN enrollments en ON en.student_id = s.id
            JOIN classes en_c ON en_c.id = en.class_id
            WHERE s.tenant_id = $3
              AND ($5::uuid IS NULL OR en_c.id = $5)
              AND (en.status ILIKE 'active' OR en.status IS NULL)
              AND s.deleted_at IS NULL
            "#,
        )
        .bind(&title_c)
        .bind(&body_c)
        .bind(tid)
        .bind(assignment_id_for_notif)
        .bind(cid)
        .execute(&ctx.pool)
        .await;

        let fcm_target = if let Some(class_uuid) = cid {
            crate::infrastructure::fcm::FcmTarget::Classes(vec![class_uuid])
        } else {
            crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()])
        };
        crate::infrastructure::fcm::trigger_fcm_push_targeted(
            tid,
            t,
            b,
            crate::infrastructure::fcm::FcmCategory::Assignment,
            assignment_id_for_notif,
            fcm_target,
        );
    }

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    Query(filter): Query<AssignmentFilterQuery>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AssignmentResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let is_teacher = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
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
        .is_some();
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
        && req_ctx
            .actor
            .as_ref()
            .map(|a| a.roles.iter().any(|r| r.name == "Siswa"))
            .unwrap_or(false);

    let mut items: Vec<AssignmentResponse> = if is_teacher {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                CASE
                    WHEN a.release_at IS NOT NULL AND a.release_at > NOW() THEN 'draft'
                    WHEN a.status = 'archived' THEN 'archived'
                    WHEN a.due_at IS NOT NULL AND a.due_at <= NOW() THEN 'closed'
                    WHEN a.release_at IS NOT NULL AND a.release_at <= NOW() AND a.status = 'draft' THEN 'published'
                    ELSE a.status
                END as status,
                (a.due_at IS NOT NULL AND a.due_at <= NOW()) as is_past_due,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id, a.session_id, a.release_at, a.allow_late_submission, a.assessment_category,
                c.name as class_name,
                sub.name as subject_name,
                COALESCE(
                    t.full_name,
                    (SELECT t3.full_name FROM teachers t3 WHERE (t3.user_id = a.created_by OR t3.id = a.created_by) LIMIT 1),
                    (SELECT u.full_name FROM users u WHERE u.id = a.created_by LIMIT 1),
                    'Guru Pengampu'
                ) as teacher_name,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT en.student_id)
                        FROM enrollments en
                        JOIN students s ON s.id = en.student_id
                        WHERE en.class_id = a.class_id
                          AND (en.status ILIKE 'active' OR en.status IS NULL)
                          AND (s.is_active = true OR s.is_active IS NULL)
                          AND (s.status ILIKE 'active' OR s.status IS NULL)
                          AND s.deleted_at IS NULL
                    ), 0
                )::bigint as total_students,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status != 'unsubmitted'
                    ), 0
                )::bigint as submitted_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status = 'graded'
                    ), 0
                )::bigint as graded_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status IN ('submitted', 'late', 'grading')
                    ), 0
                )::bigint as ungraded_count
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND (
                  a.created_by = $2 
                  OR a.teacher_id IN (SELECT id FROM teachers WHERE (user_id = $2 OR id = $2) AND tenant_id = $1)
                  OR a.teacher_id = $2
              )
              AND ($3::uuid IS NULL OR a.class_id = $3)
              AND ($4::uuid IS NULL OR a.subject_id = $4)
              AND ($5::text IS NULL OR (COALESCE(a.release_at, a.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $5::date)
              AND ($6::uuid IS NULL OR a.session_id = $6)
            ORDER BY a.updated_at DESC, a.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.date)
        .bind(filter.session_id)
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

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
                status: r.get("status"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                class_id: r.get("class_id"),
                class_name: r.get("class_name"),
                subject_name: r.get("subject_name"),
                teacher_name: r.get("teacher_name"),
                session_id: r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
                release_at: r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at").ok().flatten(),
                allow_late_submission: r.try_get::<Option<bool>, _>("allow_late_submission").ok().flatten(),
                total_students: r.try_get::<Option<i64>, _>("total_students").ok().flatten(),
                submitted_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                submission_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                graded_count: r.try_get::<Option<i64>, _>("graded_count").ok().flatten(),
                ungraded_count: r.try_get::<Option<i64>, _>("ungraded_count").ok().flatten(),
                is_past_due: r.try_get::<bool, _>("is_past_due").unwrap_or(false),
                assessment_category: r.try_get::<Option<String>, _>("assessment_category").ok().flatten(),
                has_game_questions: false,
                questions: Vec::new(),
            })
            .collect()
    } else if is_student || is_parent {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                COALESCE(
                    (
                        SELECT sub.status 
                        FROM assignment_submissions sub 
                        LEFT JOIN students s ON s.id = sub.student_id
                        WHERE sub.assignment_id = a.id 
                          AND (
                              sub.student_id = $2 
                              OR s.user_id = $2 
                              OR sub.student_id IN (SELECT id FROM students WHERE user_id = $2)
                              OR sub.student_id IN (SELECT s2.id FROM guardians g JOIN students s2 ON s2.guardian_id = g.id WHERE g.user_id = $2)
                          )
                          AND sub.status != 'unsubmitted'
                        ORDER BY sub.submitted_at DESC NULLS LAST
                        LIMIT 1
                    ),
                    CASE
                        WHEN a.release_at IS NOT NULL AND a.release_at > NOW() THEN 'draft'
                        WHEN a.due_at IS NOT NULL AND a.due_at <= NOW() AND COALESCE(a.allow_late_submission, false) = false THEN 'closed'
                        WHEN a.release_at IS NOT NULL AND a.release_at <= NOW() AND a.status = 'draft' THEN 'published'
                        ELSE a.status
                    END
                ) as status,
                (a.due_at IS NOT NULL AND a.due_at <= NOW()) as is_past_due,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id, a.session_id, a.release_at, a.allow_late_submission, a.assessment_category,
                c.name as class_name,
                sub.name as subject_name,
                COALESCE(
                    t.full_name,
                    (SELECT t2.full_name FROM class_schedules cs JOIN teachers t2 ON t2.id = cs.teacher_id WHERE cs.class_id = a.class_id AND cs.subject_id = a.subject_id LIMIT 1),
                    (SELECT t3.full_name FROM teachers t3 WHERE t3.user_id = a.created_by LIMIT 1),
                    (SELECT t4.full_name FROM teachers t4 WHERE t4.id = a.created_by LIMIT 1),
                    (SELECT t5.full_name FROM classes cl JOIN teachers t5 ON t5.id = cl.homeroom_teacher_id WHERE cl.id = a.class_id LIMIT 1),
                    (SELECT u.full_name FROM users u WHERE u.id = a.created_by LIMIT 1),
                    (SELECT t6.full_name FROM teachers t6 WHERE t6.tenant_id = a.tenant_id ORDER BY t6.created_at ASC LIMIT 1)
                ) as teacher_name,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT en.student_id)
                        FROM enrollments en
                        JOIN students s ON s.id = en.student_id
                        WHERE en.class_id = a.class_id
                          AND (en.status ILIKE 'active' OR en.status IS NULL)
                          AND (s.is_active = true OR s.is_active IS NULL)
                          AND (s.status ILIKE 'active' OR s.status IS NULL)
                          AND s.deleted_at IS NULL
                    ), 0
                )::bigint as total_students,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status != 'unsubmitted'
                    ), 0
                )::bigint as submitted_count
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND a.is_active = true
              AND (a.release_at IS NULL OR a.release_at <= NOW())
              AND (
                  a.status IN ('published', 'active', 'closed')
                  OR (a.release_at IS NOT NULL AND a.release_at <= NOW())
              )
              AND (
                  a.class_id IS NULL
                  OR a.class_id IN (
                      SELECT en.class_id FROM students s JOIN enrollments en ON en.student_id = s.id WHERE (s.user_id = $2 OR s.id = $2) AND (en.status ILIKE 'active' OR en.status IS NULL)
                      UNION
                      SELECT en.class_id FROM guardians g JOIN students s ON s.guardian_id = g.id JOIN enrollments en ON en.student_id = s.id WHERE g.user_id = $2 AND (en.status ILIKE 'active' OR en.status IS NULL)
                  )
              )
              AND ($3::uuid IS NULL OR a.class_id = $3)
              AND ($4::uuid IS NULL OR a.subject_id = $4)
              AND ($5::text IS NULL OR (COALESCE(a.release_at, a.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $5::date)
              AND ($6::uuid IS NULL OR a.session_id = $6)
            ORDER BY a.updated_at DESC, a.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.date)
        .bind(filter.session_id)
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

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
                status: r.get("status"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                class_id: r.get("class_id"),
                class_name: r.get("class_name"),
                subject_name: r.get("subject_name"),
                teacher_name: r.get("teacher_name"),
                session_id: r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
                release_at: r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at").ok().flatten(),
                allow_late_submission: r.try_get::<Option<bool>, _>("allow_late_submission").ok().flatten(),
                total_students: r.try_get::<Option<i64>, _>("total_students").ok().flatten(),
                submitted_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                submission_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                graded_count: None,
                ungraded_count: None,
                is_past_due: r.try_get::<bool, _>("is_past_due").unwrap_or(false),
                assessment_category: r.try_get::<Option<String>, _>("assessment_category").ok().flatten(),
                has_game_questions: false,
                questions: Vec::new(),
            })
            .collect()
    } else {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                CASE
                    WHEN a.release_at IS NOT NULL AND a.release_at > NOW() THEN 'draft'
                    WHEN a.status = 'archived' THEN 'archived'
                    WHEN a.due_at IS NOT NULL AND a.due_at <= NOW() THEN 'closed'
                    WHEN a.release_at IS NOT NULL AND a.release_at <= NOW() AND a.status = 'draft' THEN 'published'
                    ELSE a.status
                END as status,
                (a.due_at IS NOT NULL AND a.due_at <= NOW()) as is_past_due,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id, a.session_id, a.release_at, a.allow_late_submission, a.assessment_category,
                c.name as class_name,
                sub.name as subject_name,
                COALESCE(
                    t.full_name,
                    (SELECT t2.full_name FROM class_schedules cs JOIN teachers t2 ON t2.id = cs.teacher_id WHERE cs.class_id = a.class_id AND cs.subject_id = a.subject_id LIMIT 1),
                    (SELECT t3.full_name FROM teachers t3 WHERE t3.user_id = a.created_by LIMIT 1),
                    (SELECT t4.full_name FROM teachers t4 WHERE t4.id = a.created_by LIMIT 1),
                    (SELECT t5.full_name FROM classes cl JOIN teachers t5 ON t5.id = cl.homeroom_teacher_id WHERE cl.id = a.class_id LIMIT 1),
                    (SELECT u.full_name FROM users u WHERE u.id = a.created_by LIMIT 1),
                    (SELECT t6.full_name FROM teachers t6 WHERE t6.tenant_id = a.tenant_id ORDER BY t6.created_at ASC LIMIT 1)
                ) as teacher_name,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT en.student_id)
                        FROM enrollments en
                        JOIN students s ON s.id = en.student_id
                        WHERE en.class_id = a.class_id
                          AND (en.status ILIKE 'active' OR en.status IS NULL)
                          AND (s.is_active = true OR s.is_active IS NULL)
                          AND (s.status ILIKE 'active' OR s.status IS NULL)
                          AND s.deleted_at IS NULL
                    ), 0
                )::bigint as total_students,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status != 'unsubmitted'
                    ), 0
                )::bigint as submitted_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status = 'graded'
                    ), 0
                )::bigint as graded_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status IN ('submitted', 'late', 'grading')
                    ), 0
                )::bigint as ungraded_count
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 AND a.deleted_at IS NULL
              AND ($2::uuid IS NULL OR a.class_id = $2)
              AND ($3::uuid IS NULL OR a.subject_id = $3)
              AND ($4::text IS NULL OR (COALESCE(a.release_at, a.created_at) AT TIME ZONE 'Asia/Jakarta')::date = $4::date)
              AND ($5::uuid IS NULL OR a.session_id = $5)
            ORDER BY a.updated_at DESC, a.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(filter.class_id)
        .bind(filter.subject_id)
        .bind(filter.date)
        .bind(filter.session_id)
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

        rows.into_iter()
            .map(|r| AssignmentResponse {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
                status: r.get("status"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                class_id: r.get("class_id"),
                class_name: r.get("class_name"),
                subject_name: r.get("subject_name"),
                teacher_name: r.get("teacher_name"),
                session_id: r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
                release_at: r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at").ok().flatten(),
                allow_late_submission: r.try_get::<Option<bool>, _>("allow_late_submission").ok().flatten(),
                total_students: r.try_get::<Option<i64>, _>("total_students").ok().flatten(),
                submitted_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                submission_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                graded_count: r.try_get::<Option<i64>, _>("graded_count").ok().flatten(),
                ungraded_count: r.try_get::<Option<i64>, _>("ungraded_count").ok().flatten(),
                is_past_due: r.try_get::<bool, _>("is_past_due").unwrap_or(false),
                assessment_category: r.try_get::<Option<String>, _>("assessment_category").ok().flatten(),
                has_game_questions: false,
                questions: Vec::new(),
            })
            .collect()
    };

    if !items.is_empty() {
        let asg_ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
        if let Ok(game_ids) = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT assignment_id
            FROM assignment_questions
            WHERE assignment_id = ANY($1)
              AND UPPER(question_type) IN ('MATCHING', 'ORDERING', 'FILL_BLANK')
            "#,
        )
        .bind(&asg_ids)
        .fetch_all(&ctx.pool)
        .await
        {
            for item in &mut items {
                item.has_game_questions = game_ids.contains(&item.id);
            }
        }
    }

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let _ = sqlx::query(
        r#"
        UPDATE assignments a
        SET teacher_id = COALESCE(
                a.teacher_id,
                (SELECT cs.teacher_id FROM class_schedules cs WHERE cs.class_id = a.class_id AND cs.subject_id = a.subject_id AND cs.tenant_id = a.tenant_id LIMIT 1),
                (SELECT c.homeroom_teacher_id FROM classes c WHERE c.id = a.class_id AND c.tenant_id = a.tenant_id LIMIT 1),
                (SELECT t.id FROM teachers t WHERE t.user_id = a.created_by AND t.tenant_id = a.tenant_id LIMIT 1),
                (SELECT t.id FROM teachers t WHERE t.tenant_id = a.tenant_id ORDER BY t.created_at ASC LIMIT 1)
            )
        WHERE a.id = $1 AND a.tenant_id = $2 AND a.deleted_at IS NULL AND a.teacher_id IS NULL
        "#
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await;

    let row = sqlx::query(
        r#"
        SELECT 
            a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
            a.max_score, a.due_at, a.assignment_type,
            COALESCE(
                (
                    SELECT sub.status 
                    FROM assignment_submissions sub 
                    LEFT JOIN students s ON s.id = sub.student_id
                    WHERE sub.assignment_id = a.id 
                      AND (
                          sub.student_id = $3 
                          OR s.user_id = $3 
                          OR sub.student_id IN (SELECT id FROM students WHERE user_id = $3)
                          OR sub.student_id IN (SELECT s2.id FROM guardians g JOIN students s2 ON s2.guardian_id = g.id WHERE g.user_id = $3)
                      )
                      AND sub.status != 'unsubmitted'
                    ORDER BY sub.submitted_at DESC NULLS LAST
                    LIMIT 1
                ),
                    CASE
                        WHEN a.release_at IS NOT NULL AND a.release_at > NOW() THEN 'draft'
                        WHEN a.status = 'archived' THEN 'archived'
                        WHEN a.due_at IS NOT NULL AND a.due_at <= NOW() AND COALESCE(a.allow_late_submission, false) = false THEN 'closed'
                        WHEN a.release_at IS NOT NULL AND a.release_at <= NOW() AND a.status = 'draft' THEN 'published'
                        ELSE a.status
                    END
                ) as status,
                (a.due_at IS NOT NULL AND a.due_at <= NOW()) as is_past_due,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id, a.teacher_id, a.created_by, a.session_id, a.release_at, a.allow_late_submission, a.assessment_category,
                c.name as class_name,
                sub.name as subject_name,
                COALESCE(
                    t.full_name,
                    (SELECT t2.full_name FROM class_schedules cs JOIN teachers t2 ON t2.id = cs.teacher_id WHERE cs.class_id = a.class_id AND cs.subject_id = a.subject_id LIMIT 1),
                    (SELECT t3.full_name FROM teachers t3 WHERE t3.user_id = a.created_by LIMIT 1),
                    (SELECT t4.full_name FROM teachers t4 WHERE t4.id = a.created_by LIMIT 1),
                    (SELECT t5.full_name FROM classes cl JOIN teachers t5 ON t5.id = cl.homeroom_teacher_id WHERE cl.id = a.class_id LIMIT 1),
                    (SELECT u.full_name FROM users u WHERE u.id = a.created_by LIMIT 1),
                    (SELECT t6.full_name FROM teachers t6 WHERE t6.tenant_id = a.tenant_id ORDER BY t6.created_at ASC LIMIT 1)
                ) as teacher_name,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT en.student_id)
                        FROM enrollments en
                        JOIN students s ON s.id = en.student_id
                        WHERE en.class_id = a.class_id
                          AND (en.status ILIKE 'active' OR en.status IS NULL)
                          AND (s.is_active = true OR s.is_active IS NULL)
                          AND (s.status ILIKE 'active' OR s.status IS NULL)
                          AND s.deleted_at IS NULL
                    ), 0
                )::bigint as total_students,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status != 'unsubmitted'
                    ), 0
                )::bigint as submitted_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status = 'graded'
                    ), 0
                )::bigint as graded_count,
                COALESCE(
                    (
                        SELECT COUNT(DISTINCT sub.student_id)
                        FROM assignment_submissions sub
                        WHERE sub.assignment_id = a.id
                          AND sub.status IN ('submitted', 'late', 'grading')
                    ), 0
                )::bigint as ungraded_count
        FROM assignments a
        LEFT JOIN classes c ON c.id = a.class_id
        LEFT JOIN subjects sub ON sub.id = a.subject_id
        LEFT JOIN teachers t ON t.id = a.teacher_id
        WHERE a.id = $1 AND a.tenant_id = $2 AND a.deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .bind(req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default())
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

    match row {
        Some(r) => {
            let tenant_id: Uuid = r.get("tenant_id");
            let class_id: Option<Uuid> = r.get("class_id");
            let teacher_id: Option<Uuid> = r.get("teacher_id");
            let created_by: Option<Uuid> = r.get("created_by");

            // Strict Cross-Class and Multi-Tenant Access Verification
            crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
                &ctx.pool, &req_ctx, tenant_id, class_id, teacher_id, created_by,
            )
            .await?;

            let is_student = req_ctx
                .actor
                .as_ref()
                .map(|a| a.roles.iter().any(|r| r.name == "Siswa"))
                .unwrap_or(false);

            let questions = fetch_assignment_questions(&ctx.pool, id, is_student)
                .await
                .unwrap_or_default();

            let raw_teacher_name = r.get::<Option<String>, _>("teacher_name");
            let resolved_teacher_name = match raw_teacher_name {
                Some(ref tn) if !tn.trim().is_empty() && !tn.eq_ignore_ascii_case("Guru Pengampu") => Some(tn.clone()),
                _ => {
                    sqlx::query_scalar::<_, String>(
                        "SELECT full_name FROM teachers WHERE tenant_id = $1 ORDER BY created_at ASC LIMIT 1",
                    )
                    .bind(tenant_id)
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten()
                    .or_else(|| Some("Guru Mata Pelajaran".to_string()))
                }
            };

            let resp = AssignmentResponse {
                id: r.get("id"),
                tenant_id,
                lesson_id: r.get("lesson_id"),
                title: r.get("title"),
                description: r.get("description"),
                instructions: r.get("instructions"),
                max_score: r.get("max_score"),
                due_at: r.get("due_at"),
                assignment_type: r.get("assignment_type"),
                status: r.get("status"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                class_id,
                class_name: r.get("class_name"),
                subject_name: r.get("subject_name"),
                teacher_name: resolved_teacher_name,
                session_id: r.try_get::<Option<Uuid>, _>("session_id").ok().flatten(),
                release_at: r.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("release_at").ok().flatten(),
                allow_late_submission: r.try_get::<Option<bool>, _>("allow_late_submission").ok().flatten(),
                total_students: r.try_get::<Option<i64>, _>("total_students").ok().flatten(),
                submitted_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                submission_count: r.try_get::<Option<i64>, _>("submitted_count").ok().flatten(),
                graded_count: r.try_get::<Option<i64>, _>("graded_count").ok().flatten(),
                ungraded_count: r.try_get::<Option<i64>, _>("ungraded_count").ok().flatten(),
                is_past_due: r.try_get::<bool, _>("is_past_due").unwrap_or(false),
                assessment_category: r.try_get::<Option<String>, _>("assessment_category").ok().flatten(),
                has_game_questions: questions.iter().any(|q| is_interactive_game_type(&q.question_type)),
                questions,
            };
            Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
        }
        None => Err(ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::AssignmentNotFound,
                format!("Assignment {} not found", id),
            ),
            &req_ctx.request_id,
        )),
    }
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        title: payload.title,
        description: payload.description,
        instructions: payload.instructions,
        max_score: payload.max_score,
        due_at: payload.due_at,
    };

    let assignment = ctx
        .update_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let target_status = payload.status.as_deref();
    let is_activating = target_status == Some("published") || target_status == Some("active");

    if payload.release_at.is_some()
        || target_status.is_some()
        || payload.subject_id.is_some()
        || payload.teacher_id.is_some()
        || payload.class_id.is_some()
        || payload.assessment_category.is_some()
    {
        crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;
        let norm_cat = payload.assessment_category.as_deref().map(|c| {
            crate::presentation::learning::grade_sync::normalize_assessment_category(
                Some(c),
                Some(&assignment.title),
            )
        });
        let (new_status, new_release_at, mark_dispatched) = if is_activating {
            (Some("published"), Some(chrono::Utc::now()), true)
        } else if let Some(release_at) = payload.release_at {
            let is_sched = release_at > chrono::Utc::now();
            let s = if is_sched { "draft" } else { "published" };
            (Some(s), Some(release_at), !is_sched)
        } else if let Some(ts) = target_status {
            (Some(ts), None, false)
        } else {
            (None, None, false)
        };

        let _ = sqlx::query(
            r#"UPDATE assignments 
               SET status = COALESCE($1, status), 
                   release_at = COALESCE($2, release_at), 
                   notification_dispatched = CASE WHEN $3 = true THEN true ELSE notification_dispatched END,
                   subject_id = COALESCE($5, subject_id),
                   teacher_id = COALESCE($6, teacher_id),
                   class_id = COALESCE($7, class_id),
                   assessment_category = COALESCE($8, assessment_category),
                   is_active = true,
                   updated_at = NOW() 
               WHERE id = $4"#
        )
        .bind(new_status)
        .bind(new_release_at)
        .bind(mark_dispatched)
        .bind(id)
        .bind(payload.subject_id)
        .bind(payload.teacher_id)
        .bind(payload.class_id)
        .bind(norm_cat)
        .execute(&ctx.pool)
        .await;

        if is_activating {
            let notif_info = sqlx::query(
                r#"
                SELECT a.title, a.class_id, c.name as class_name,
                       COALESCE(ut.full_name, 'Guru Pengampu') as teacher_name
                FROM assignments a
                LEFT JOIN classes c ON c.id = a.class_id
                LEFT JOIN teachers t ON t.id = a.teacher_id
                LEFT JOIN users ut ON ut.id = t.user_id
                WHERE a.id = $1
                "#
            )
            .bind(id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            if let Some(n) = notif_info {
                use sqlx::Row;
                let title: String = n.get("title");
                let class_id: Option<Uuid> = n.try_get("class_id").ok().flatten();
                let class_name: Option<String> = n.try_get("class_name").ok().flatten();
                let teacher_name: String = n.get("teacher_name");

                let notif_title = format!("📝 Tugas Baru: {}", title);
                let notif_body = if let Some(ref c_name) = class_name {
                    format!("{} telah menerbitkan tugas baru untuk kelas {}. Kerjakan sekarang!", teacher_name, c_name)
                } else {
                    format!("{} telah menerbitkan tugas baru: {}. Buka aplikasi untuk mengerjakan!", teacher_name, title)
                };

                if let Some(cid) = class_id {
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                        SELECT 
                            gen_random_uuid(), s.tenant_id, s.user_id, $1, $2, 'ASSIGNMENT', 'in_app', 'assignment', $3, FALSE, NOW()
                        FROM students s
                        JOIN enrollments en ON en.student_id = s.id
                        WHERE en.class_id = $4 AND (en.status ILIKE 'active' OR en.status IS NULL)
                        "#
                    )
                    .bind(&notif_title)
                    .bind(&notif_body)
                    .bind(id)
                    .bind(cid)
                    .execute(&ctx.pool)
                    .await;

                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        req_ctx.tenant_id,
                        notif_title,
                        notif_body,
                        crate::infrastructure::fcm::FcmCategory::Assignment,
                        id,
                        crate::infrastructure::fcm::FcmTarget::Classes(vec![cid]),
                    );
                }
            }
        }
    }

    if let Some(questions) = payload.questions {
        let _ = sqlx::query("DELETE FROM assignment_questions WHERE assignment_id = $1")
            .bind(id)
            .execute(&ctx.pool)
            .await;

        let _ = persist_assignment_questions(&ctx.pool, req_ctx.tenant_id, id, questions).await;
    }

    let questions = fetch_assignment_questions(&ctx.pool, id, false)
        .await
        .unwrap_or_default();
    let mut resp = AssignmentResponse::from(assignment);
    resp.has_game_questions = questions
        .iter()
        .any(|q| is_interactive_game_type(&q.question_type));
    resp.questions = questions;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .publish_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // FCM saat tugas di-PUBLISH (kasus guru buat draft lalu publish).
    {
        let target_class_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT class_id FROM assignments WHERE id = $1 AND tenant_id = $2",
        )
        .bind(assignment.id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        let t = format!("📝 Tugas Dipublish: {}", assignment.title);
        let b = "Tugas baru sudah dipublish. Buka aplikasi untuk mengerjakan.".to_string();
        let fcm_target = if let Some(cid) = target_class_id {
            crate::infrastructure::fcm::FcmTarget::Class(cid)
        } else {
            crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()])
        };
        crate::infrastructure::fcm::trigger_fcm_push_targeted(
            req_ctx.tenant_id,
            t,
            b,
            crate::infrastructure::fcm::FcmCategory::Assignment,
            assignment.id,
            fcm_target,
        );
    }

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn close(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CloseAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .close_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn archive(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = ArchiveAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .archive_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn delete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let command = DeleteAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

/// Upload an assignment worksheet/photo/document attachment for student submissions or teacher tasks.
async fn upload_submission_file(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    multipart: Multipart,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;

    let has_perm = require_permission(&req_ctx.actor, Permission::LearningAssignmentSubmit).is_ok()
        || require_permission(&req_ctx.actor, Permission::LearningAssignmentCreate).is_ok()
        || require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).is_ok();
    if !has_perm {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    crate::presentation::learning::materials::controller::store_uploaded_file(
        &ctx,
        &req_ctx,
        multipart,
        &["pdf", "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "doc", "docx", "txt"],
        "PDF, gambar (JPG/PNG/WEBP/HEIC), dan dokumen (DOC/DOCX/TXT)",
    )
    .await
}

async fn submit(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubmitAssignmentRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentSubmit).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let assignment = sqlx::query(
        r#"
        SELECT
            a.tenant_id,
            a.class_id,
            a.teacher_id,
            a.created_by,
            COALESCE(a.title, 'Tugas') AS title,
            (a.due_at IS NOT NULL AND a.due_at <= NOW()) AS is_past_due,
            c.name AS class_name,
            COALESCE(t.user_id, a.created_by) AS teacher_user_id
        FROM assignments a
        LEFT JOIN classes c ON c.id = a.class_id
        LEFT JOIN teachers t ON t.id = a.teacher_id
        WHERE a.id = $1 AND a.tenant_id = $2 AND a.deleted_at IS NULL
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
                school_core::common::error_code::ErrorCode::AssignmentNotFound,
                format!("Assignment {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    let assignment_tenant_id: Uuid = assignment.get("tenant_id");
    let assignment_class_id: Option<Uuid> = assignment.get("class_id");
    let assignment_teacher_id: Option<Uuid> = assignment.get("teacher_id");
    let assignment_created_by: Option<Uuid> = assignment.get("created_by");
    let assignment_title: String = assignment.get("title");
    let assignment_is_past_due: bool = assignment.try_get("is_past_due").unwrap_or(false);
    let assignment_class_name: Option<String> = assignment.try_get("class_name").ok().flatten();
    let assignment_teacher_user_id: Option<Uuid> =
        assignment.try_get("teacher_user_id").ok().flatten();

    // Verify student belongs to the class of this assignment
    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        assignment_tenant_id,
        assignment_class_id,
        assignment_teacher_id,
        assignment_created_by,
    )
    .await?;

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

    let command = SubmitAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        student_id,
        content: payload.content.clone(),
        file_url: payload.file_url.clone(),
    };

    let submission = ctx
        .submit_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    ensure_interaction_config_column(&ctx.pool).await;
    let q_meta_rows = sqlx::query(
        "SELECT id, COALESCE(question_type, 'MULTIPLE_CHOICE') as question_type, COALESCE(points, 10) as points, interaction_config FROM assignment_questions WHERE assignment_id = $1",
    )
    .bind(id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let mut has_game_question = false;
    let mut has_essay = false;
    let mut total_max_points: i32 = 0;
    let mut q_meta_map: std::collections::HashMap<Uuid, (String, i32, Option<serde_json::Value>)> =
        std::collections::HashMap::new();
    let mut mc_q_ids: Vec<Uuid> = Vec::new();

    for r in &q_meta_rows {
        let qid: Uuid = r.get("id");
        let qtype: String = r.get("question_type");
        let pts: i32 = r.get::<i32, _>("points").max(1);
        let cfg: Option<serde_json::Value> = r.try_get("interaction_config").ok().flatten();
        total_max_points += pts;
        if is_interactive_game_type(&qtype) {
            has_game_question = true;
        } else if qtype.eq_ignore_ascii_case("MULTIPLE_CHOICE") || qtype.eq_ignore_ascii_case("PG") {
            mc_q_ids.push(qid);
        } else {
            has_essay = true;
        }
        q_meta_map.insert(qid, (qtype, pts, cfg));
    }

    let auto_grade_all = !q_meta_rows.is_empty()
        && has_game_question
        && !has_essay
        && payload.file_url.as_deref().unwrap_or("").trim().is_empty();

    let mut correct_choice_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    if auto_grade_all && !mc_q_ids.is_empty() {
        if let Ok(rows) = sqlx::query(
            "SELECT id FROM assignment_question_choices WHERE question_id = ANY($1) AND is_correct = true",
        )
        .bind(&mc_q_ids)
        .fetch_all(&ctx.pool)
        .await
        {
            for cr in rows {
                correct_choice_ids.insert(cr.get("id"));
            }
        }
    }

    let mut earned_points_sum: i32 = 0;
    if let Some(answers) = payload.answers {
        for ans in answers {
            let mut earned = 0i32;
            let mut is_auto_graded = false;
            if let Some((qtype, max_pts, cfg)) = q_meta_map.get(&ans.question_id) {
                if is_interactive_game_type(qtype) {
                    if let Some(ref cfg_val) = cfg {
                        let grade_res = grade_interactive_answer(
                            qtype,
                            cfg_val,
                            ans.text_answer.as_deref(),
                            *max_pts,
                        );
                        earned = grade_res.points_earned;
                        is_auto_graded = true;
                    }
                } else if auto_grade_all
                    && (qtype.eq_ignore_ascii_case("MULTIPLE_CHOICE")
                        || qtype.eq_ignore_ascii_case("PG"))
                {
                    if let Some(chosen_id) = ans.chosen_choice_id {
                        if correct_choice_ids.contains(&chosen_id) {
                            earned = *max_pts;
                        }
                    }
                    is_auto_graded = true;
                }
            }
            earned_points_sum += earned;

            let _ = sqlx::query(
                r#"
                INSERT INTO assignment_submission_answers (
                    submission_id, question_id, chosen_choice_id, text_answer, points_earned, graded_at
                )
                VALUES ($1, $2, $3, $4, $5, CASE WHEN $6 THEN NOW() ELSE NULL END)
                ON CONFLICT (submission_id, question_id) DO UPDATE SET
                    chosen_choice_id = EXCLUDED.chosen_choice_id,
                    text_answer = EXCLUDED.text_answer,
                    points_earned = EXCLUDED.points_earned,
                    graded_at = CASE WHEN $6 THEN NOW() ELSE assignment_submission_answers.graded_at END
                "#,
            )
            .bind(submission.id)
            .bind(ans.question_id)
            .bind(ans.chosen_choice_id)
            .bind(&ans.text_answer)
            .bind(earned)
            .bind(is_auto_graded)
            .execute(&ctx.pool)
            .await;
        }
    }

    let mut final_submission = submission;
    if auto_grade_all && total_max_points > 0 {
        let auto_score = calculate_total_percentage_score(earned_points_sum, total_max_points);
        let auto_feedback = "Penilaian otomatis Misi Game Interaktif";
        let _ = sqlx::query(
            r#"
            UPDATE assignment_submissions
            SET score = $1, status = 'graded', graded_at = NOW(), feedback = $2
            WHERE id = $3
            "#,
        )
        .bind(auto_score)
        .bind(auto_feedback)
        .bind(final_submission.id)
        .execute(&ctx.pool)
        .await;

        final_submission.score = Some(auto_score);
        final_submission.status = "graded".to_string();
        final_submission.feedback = Some(auto_feedback.to_string());

        let asg_title: String = assignment.get("title");
        let xp_val = if earned_points_sum > 0 { earned_points_sum } else { auto_score };
        let xp_desc = format!("Misi Game Tugas: {} ({} Poin)", asg_title, xp_val);
        let _ = crate::presentation::gamification::controller::sync_scored_xp_for_student(
            &ctx.pool,
            req_ctx.tenant_id,
            final_submission.student_id,
            "ASSIGNMENT_SCORE",
            final_submission.id,
            xp_val,
            &xp_desc,
        )
        .await;

        crate::presentation::learning::grade_sync::sync_assignment_submission_to_gradebook(
            &ctx.pool,
            req_ctx.tenant_id,
            final_submission.id,
        )
        .await;
    }

    let needs_manual_grading = final_submission.status != "graded";
    let submission_id = final_submission.id;
    let submission_student_id = final_submission.student_id;
    let submission_status = final_submission.status.clone();

    let mut resp = SubmissionResponse::from(final_submission);

    if needs_manual_grading {
        if let Some(teacher_uid) = assignment_teacher_user_id.filter(|uid| *uid != actor_id) {
            let student_name = sqlx::query_scalar::<_, String>(
                r#"
                SELECT COALESCE(NULLIF(TRIM(u.full_name), ''), 'Warga Belajar')
                FROM users u
                LEFT JOIN students s ON s.user_id = u.id
                WHERE s.id = $1 OR u.id = $2
                ORDER BY (s.id = $1) DESC
                LIMIT 1
                "#,
            )
            .bind(submission_student_id)
            .bind(actor_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "Warga Belajar".to_string());

            let ungraded_total: i64 = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM assignment_submissions WHERE assignment_id = $1 AND status IN ('submitted', 'late', 'grading')",
            )
            .bind(id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap_or(1)
            .max(1);

            let class_label = assignment_class_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Kelas Anda");
            let status_note = if submission_status == "late" {
                " (terlambat)"
            } else {
                ""
            };
            let notif_title = if assignment_is_past_due {
                format!("⚠️ Tugas Lewat Tenggat Siap Dinilai: {}", assignment_title)
            } else {
                format!("📝 Tugas Baru Siap Dinilai: {}", assignment_title)
            };
            let notif_body = format!(
                "{}{} dari {} telah mengumpulkan \"{}\". Total {} jawaban menunggu penilaian Anda.",
                student_name, status_note, class_label, assignment_title, ungraded_total
            );
            let priority_label = if assignment_is_past_due { "HIGH" } else { "NORMAL" };

            if let Ok(res) = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type, channel,
                    reference_type, reference_id, is_read, dedup_key, scheduled_at, is_urgent, priority, created_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, 'GRADING_WARNING', 'in_app',
                    'assignment', $6, false,
                    CONCAT('SUB_READY:', $7::text, ':', to_char(NOW() AT TIME ZONE 'Asia/Jakarta', 'YYYYMMDDHH24')),
                    NOW(), $8, $9, NOW()
                )
                ON CONFLICT (tenant_id, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(req_ctx.tenant_id)
            .bind(teacher_uid)
            .bind(&notif_title)
            .bind(&notif_body)
            .bind(id)
            .bind(submission_id)
            .bind(assignment_is_past_due)
            .bind(priority_label)
            .execute(&ctx.pool)
            .await
            {
                if res.rows_affected() > 0 {
                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        req_ctx.tenant_id,
                        notif_title,
                        notif_body,
                        crate::infrastructure::fcm::FcmCategory::Assignment,
                        id,
                        crate::infrastructure::fcm::FcmTarget::User(teacher_uid),
                    );
                }
            }
        }
    }

    let answer_rows = sqlx::query(
        r#"
        SELECT 
            ans.question_id,
            COALESCE(q.question_text, '') as question_text,
            COALESCE(q.question_type, 'MULTIPLE_CHOICE') as question_type,
            COALESCE(q.points, 10) as max_points,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            c.is_correct as is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback,
            q.interaction_config
        FROM assignment_submission_answers ans
        LEFT JOIN assignment_questions q ON q.id = ans.question_id
        LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.submission_id = $1
        ORDER BY COALESCE(q.order_index, 0) ASC, ans.created_at ASC
        "#,
    )
    .bind(resp.id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    resp.answers = answer_rows
        .into_iter()
        .map(|a| SubmissionAnswerDetailDto {
            question_id: a.get("question_id"),
            question_text: a.get("question_text"),
            question_type: a.get("question_type"),
            max_points: a.get("max_points"),
            chosen_choice_id: a.get("chosen_choice_id"),
            chosen_choice_text: a.get("chosen_choice_text"),
            is_correct: a.get("is_correct"),
            text_answer: a.get("text_answer"),
            points_earned: a.get::<Option<i32>, _>("points_earned").unwrap_or(0),
            teacher_feedback: a.get("teacher_feedback"),
            interaction_config: a.try_get("interaction_config").ok().flatten(),
        })
        .collect();

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}

async fn get_submissions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<SubmissionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::presentation::learning::grade_sync::ensure_grade_sync_schema(&ctx.pool).await;

    let asg_row = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by, description FROM assignments WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
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
                school_core::common::error_code::ErrorCode::AssignmentNotFound,
                format!("Assignment {} not found", id),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        asg_row.get("tenant_id"),
        asg_row.get("class_id"),
        asg_row.get("teacher_id"),
        asg_row.get("created_by"),
    )
    .await?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let is_student = req_ctx
        .actor
        .as_ref()
        .map(|a| a.roles.iter().any(|r| r.name == "Siswa"))
        .unwrap_or(false);
    let is_parent = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles
                .iter()
                .any(|r| r.name == "Wali Murid" || r.name == "Orang Tua")
        })
        .unwrap_or(false);

    let rows = if is_student || is_parent {
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
                sub.id, sub.tenant_id, sub.assignment_id, sub.student_id,
                sub.content, sub.file_url, sub.submitted_at, sub.status,
                sub.score, sub.feedback, sub.graded_at, sub.graded_by,
                COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
                COALESCE(s.nisn, u.username, '-') as student_nisn,
                COALESCE(s.user_id, u.id) as student_user_id
            FROM assignment_submissions sub
            LEFT JOIN students s ON s.id = sub.student_id OR s.user_id = sub.student_id
            LEFT JOIN users u ON u.id = sub.student_id OR u.id = s.user_id
            WHERE sub.assignment_id = $1
              AND (
                  sub.student_id = $2
                  OR s.user_id = $2
                  OR ($3::uuid IS NOT NULL AND sub.student_id = $3)
              )
            ORDER BY sub.submitted_at DESC
            "#,
        )
        .bind(id)
        .bind(actor_id)
        .bind(student_db_id)
        .fetch_all(&ctx.pool)
        .await
    } else {
        let mut resolved_class_id: Option<Uuid> = asg_row.get("class_id");
        if resolved_class_id.is_none() {
            if let Some(desc) = asg_row.get::<Option<String>, _>("description") {
                let class_part = desc.split('•').nth(1).map(|s| s.trim()).unwrap_or("");
                if !class_part.is_empty() && !class_part.eq_ignore_ascii_case("Semua Rombel") {
                    if let Ok(Some(cid)) = sqlx::query_scalar::<_, Uuid>(
                        r#"SELECT id FROM classes WHERE tenant_id = $1 AND (name ILIKE $2 OR REPLACE(name, ' ', '') ILIKE REPLACE($2, ' ', '')) LIMIT 1"#,
                    )
                    .bind(req_ctx.tenant_id)
                    .bind(class_part)
                    .fetch_optional(&ctx.pool)
                    .await
                    {
                        let _ = sqlx::query("UPDATE assignments SET class_id = $1 WHERE id = $2")
                            .bind(cid)
                            .bind(id)
                            .execute(&ctx.pool)
                            .await;
                        resolved_class_id = Some(cid);
                    }
                }
            }
        }

        if let Some(class_id) = resolved_class_id {
            sqlx::query(
                r#"
                SELECT 
                    student_id,
                    id,
                    tenant_id,
                    assignment_id,
                    content,
                    file_url,
                    submitted_at,
                    status,
                    score,
                    feedback,
                    graded_at,
                    graded_by,
                    student_name,
                    student_nisn,
                    student_user_id
                FROM (
                    SELECT DISTINCT ON (s.id)
                        s.id as student_id,
                        COALESCE(sub.id, gen_random_uuid()) as id,
                        COALESCE(sub.tenant_id, s.tenant_id) as tenant_id,
                        $1 as assignment_id,
                        sub.content,
                        sub.file_url,
                        COALESCE(sub.submitted_at, NOW()) as submitted_at,
                        COALESCE(sub.status, 'unsubmitted') as status,
                        sub.score,
                        sub.feedback,
                        sub.graded_at,
                        sub.graded_by,
                        s.full_name as student_name,
                        COALESCE(s.nisn, u.username, '-') as student_nisn,
                        s.user_id as student_user_id
                    FROM enrollments en
                    JOIN students s ON s.id = en.student_id
                    LEFT JOIN users u ON u.id = s.user_id
                    LEFT JOIN assignment_submissions sub ON sub.assignment_id = $1 AND (sub.student_id = s.id OR (s.user_id IS NOT NULL AND sub.student_id = s.user_id))
                    JOIN classes en_c ON en_c.id = en.class_id
                    WHERE (en_c.id = $2 OR sub.id IS NOT NULL)
                      AND (en.status ILIKE 'active' OR en.status IS NULL OR sub.id IS NOT NULL)
                      AND s.deleted_at IS NULL
                    ORDER BY s.id, (sub.status IS NOT NULL AND sub.status != 'unsubmitted') DESC, sub.submitted_at DESC NULLS LAST
                ) subquery
                ORDER BY 
                    CASE WHEN status = 'submitted' THEN 1
                         WHEN status = 'graded' THEN 2
                         ELSE 3 END ASC,
                    submitted_at DESC NULLS LAST,
                    student_name ASC
                "#,
            )
            .bind(id)
            .bind(class_id)
            .fetch_all(&ctx.pool)
            .await
        } else {
            // Strictly return students who actually submitted - NEVER return whole school
            sqlx::query(
                r#"
                SELECT 
                    sub.student_id,
                    sub.id,
                    sub.tenant_id,
                    sub.assignment_id,
                    sub.content,
                    sub.file_url,
                    COALESCE(sub.submitted_at, NOW()) as submitted_at,
                    sub.status,
                    sub.score,
                    sub.feedback,
                    sub.graded_at,
                    sub.graded_by,
                    COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
                    COALESCE(s.nisn, u.username, '-') as student_nisn,
                    COALESCE(s.user_id, u.id) as student_user_id
                FROM assignment_submissions sub
                JOIN students s ON s.id = sub.student_id OR s.user_id = sub.student_id
                LEFT JOIN users u ON u.id = s.user_id
                WHERE sub.assignment_id = $1
                  AND sub.tenant_id = $2
                  AND sub.status != 'unsubmitted'
                ORDER BY 
                    CASE WHEN sub.status = 'submitted' THEN 1
                         WHEN sub.status = 'graded' THEN 2
                         ELSE 3 END ASC,
                    sub.submitted_at DESC NULLS LAST,
                    student_name ASC
                "#,
            )
            .bind(id)
            .bind(req_ctx.tenant_id)
            .fetch_all(&ctx.pool)
            .await
        }
    }
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Comprehensive answer matching: Query all submission answers associated with this assignment
    let answer_rows = sqlx::query(
        r#"
        SELECT 
            ans.submission_id,
            sub.student_id,
            ans.question_id,
            COALESCE(q.question_text, '') as question_text,
            COALESCE(q.question_type, 'MULTIPLE_CHOICE') as question_type,
            COALESCE(q.points, 10) as max_points,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            c.is_correct as is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback,
            q.interaction_config
        FROM assignment_submission_answers ans
        JOIN assignment_submissions sub ON sub.id = ans.submission_id
        LEFT JOIN assignment_questions q ON q.id = ans.question_id
        LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
        WHERE sub.assignment_id = $1
        ORDER BY COALESCE(q.order_index, 0) ASC, ans.created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let items = rows
        .into_iter()
        .map(|r| {
            let sub_id: Uuid = r.get("id");
            let student_id: Uuid = r.get("student_id");
            let student_user_id: Option<Uuid> = r.try_get("student_user_id").ok().flatten();
            let sub_status: String = r.get("status");
            let sub_score: Option<i32> = r.get("score");
            let is_graded = sub_status.eq_ignore_ascii_case("graded") || sub_score.is_some();
            let answers = answer_rows
                .iter()
                .filter(|a| {
                    let a_sub_id: Uuid = a.get("submission_id");
                    let a_student_id: Uuid = a.get("student_id");
                    a_sub_id == sub_id || a_student_id == student_id || (student_user_id.is_some() && Some(a_student_id) == student_user_id)
                })
                .map(|a| {
                    let question_type: String = a.get("question_type");
                    let max_points: i32 = a.get("max_points");
                    let is_correct: Option<bool> = a.get("is_correct");
                    let raw_points = a.get::<Option<i32>, _>("points_earned").unwrap_or(0);
                    let is_mc = question_type.eq_ignore_ascii_case("MULTIPLE_CHOICE") || question_type.eq_ignore_ascii_case("PG");
                    let effective_points = if raw_points == 0 && is_graded && is_mc && is_correct == Some(true) {
                        max_points.max(1)
                    } else {
                        raw_points
                    };
                    SubmissionAnswerDetailDto {
                        question_id: a.get("question_id"),
                        question_text: a.get("question_text"),
                        question_type,
                        max_points,
                        chosen_choice_id: a.get("chosen_choice_id"),
                        chosen_choice_text: a.get("chosen_choice_text"),
                        is_correct,
                        text_answer: a.get("text_answer"),
                        points_earned: effective_points,
                        teacher_feedback: a.get("teacher_feedback"),
                        interaction_config: a.try_get("interaction_config").ok().flatten(),
                    }
                })
                .collect();

            SubmissionResponse {
                id: sub_id,
                tenant_id: r.get("tenant_id"),
                assignment_id: r.get("assignment_id"),
                student_id: r.get("student_id"),
                content: r.get("content"),
                file_url: r.get("file_url"),
                submitted_at: r.get("submitted_at"),
                status: sub_status,
                score: sub_score,
                feedback: r.get("feedback"),
                graded_at: r.get("graded_at"),
                graded_by: r.get("graded_by"),
                student_name: r.get("student_name"),
                student_nisn: r.get("student_nisn"),
                student_user_id: r.get("student_user_id"),
                answers,
            }
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn grade(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((assignment_id, submission_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GradeSubmissionRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let grader_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let sub_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM assignment_submissions WHERE id = $1 AND assignment_id = $2 AND tenant_id = $3)",
    )
    .bind(submission_id)
    .bind(assignment_id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(false);
    if !sub_exists {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                school_core::common::error_code::ErrorCode::ResourceNotFound,
                "Submission tugas tidak ditemukan pada penugasan ini.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let asg_row = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM assignments WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
    )
    .bind(assignment_id)
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
                school_core::common::error_code::ErrorCode::AssignmentNotFound,
                format!("Assignment {} not found", assignment_id),
            ),
            &req_ctx.request_id,
        )
    })?;

    crate::authorization_helpers::AuthorizationScope::verify_learning_resource_access(
        &ctx.pool,
        &req_ctx,
        asg_row.get("tenant_id"),
        asg_row.get("class_id"),
        asg_row.get("teacher_id"),
        asg_row.get("created_by"),
    )
    .await?;

    let has_explicit_grades = payload
        .answer_grades
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if let Some(ref answer_grades) = payload.answer_grades {
        for ag in answer_grades {
            let _ = sqlx::query(
                r#"
                UPDATE assignment_submission_answers
                SET points_earned = $1, teacher_feedback = COALESCE($2, teacher_feedback), graded_at = NOW(), graded_by = $3
                WHERE submission_id = $4 AND question_id = $5
                "#,
            )
            .bind(ag.points_earned)
            .bind(&ag.teacher_feedback)
            .bind(grader_id)
            .bind(submission_id)
            .bind(ag.question_id)
            .execute(&ctx.pool)
            .await;
        }
    }

    if !has_explicit_grades {
        // Auto-sync per-question points in DB when grading client (e.g. legacy client) didn't pass answer_grades:
        // 1) Multiple Choice gets max_points when correct, else 0.
        // 2) Remaining score is distributed proportionally to non-MC (Essay) answers up to their max_points.
        let existing_rows = sqlx::query(
            r#"
            SELECT
                ans.question_id,
                COALESCE(q.question_type, 'MULTIPLE_CHOICE') as question_type,
                COALESCE(q.points, 10) as max_points,
                COALESCE(c.is_correct, false) as is_correct,
                COALESCE(ans.points_earned, 0) as current_points
            FROM assignment_submission_answers ans
            LEFT JOIN assignment_questions q ON q.id = ans.question_id
            LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
            WHERE ans.submission_id = $1
            ORDER BY COALESCE(q.order_index, 0) ASC, ans.created_at ASC
            "#,
        )
        .bind(submission_id)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

        let mut pg_total: i32 = 0;
        let mut essay_items: Vec<(Uuid, i32, i32)> = Vec::new();
        for row in &existing_rows {
            let qid: Uuid = row.get("question_id");
            let qtype: String = row.get("question_type");
            let max_pts: i32 = row.get("max_points");
            let is_corr: bool = row.get("is_correct");
            let cur_pts: i32 = row.get("current_points");
            let is_mc = qtype.eq_ignore_ascii_case("MULTIPLE_CHOICE") || qtype.eq_ignore_ascii_case("PG");
            if is_mc {
                let earned = if is_corr { max_pts.max(1) } else { 0 };
                pg_total += earned;
                let _ = sqlx::query(
                    r#"
                    UPDATE assignment_submission_answers
                    SET points_earned = $1, graded_at = NOW(), graded_by = $2
                    WHERE submission_id = $3 AND question_id = $4
                    "#,
                )
                .bind(earned)
                .bind(grader_id)
                .bind(submission_id)
                .bind(qid)
                .execute(&ctx.pool)
                .await;
            } else if is_interactive_game_type(&qtype) {
                pg_total += cur_pts;
            } else {
                essay_items.push((qid, max_pts.max(1), cur_pts));
            }
        }

        if !essay_items.is_empty() {
            let all_essays_zero = essay_items.iter().all(|(_, _, cur)| *cur == 0);
            if all_essays_zero {
                let mut remaining = (payload.score - pg_total).max(0);
                let total_essay_max: i32 = essay_items.iter().map(|(_, m, _)| *m).sum::<i32>().max(1);
                let len = essay_items.len();
                for (i, (qid, max_pts, _)) in essay_items.into_iter().enumerate() {
                    let alloc = if i + 1 == len {
                        remaining.min(max_pts)
                    } else {
                        ((remaining as i64 * max_pts as i64) / total_essay_max as i64) as i32
                    }
                    .clamp(0, max_pts);
                    remaining = (remaining - alloc).max(0);
                    let _ = sqlx::query(
                        r#"
                        UPDATE assignment_submission_answers
                        SET points_earned = $1, graded_at = NOW(), graded_by = $2
                        WHERE submission_id = $3 AND question_id = $4
                        "#,
                    )
                    .bind(alloc)
                    .bind(grader_id)
                    .bind(submission_id)
                    .bind(qid)
                    .execute(&ctx.pool)
                    .await;
                }
            }
        }
    }

    let command = GradeSubmissionCommand {
        tenant_id: req_ctx.tenant_id,
        submission_id,
        score: payload.score,
        feedback: payload.feedback.clone(),
        graded_by: grader_id,
    };

    let submission = ctx
        .grade_submission
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let student_info = sqlx::query(
        r#"
        SELECT 
            COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
            COALESCE(s.nisn, u.username, '-') as student_nisn
        FROM assignment_submissions sub
        LEFT JOIN students s ON s.id = sub.student_id OR s.user_id = sub.student_id
        LEFT JOIN users u ON u.id = sub.student_id
        WHERE sub.id = $1
        LIMIT 1
        "#,
    )
    .bind(submission_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let answer_rows = sqlx::query(
        r#"
        SELECT 
            ans.question_id,
            COALESCE(q.question_text, '') as question_text,
            COALESCE(q.question_type, 'MULTIPLE_CHOICE') as question_type,
            COALESCE(q.points, 10) as max_points,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            c.is_correct as is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback,
            q.interaction_config
        FROM assignment_submission_answers ans
        LEFT JOIN assignment_questions q ON q.id = ans.question_id
        LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.submission_id = $1
        ORDER BY COALESCE(q.order_index, 0) ASC, ans.created_at ASC
        "#,
    )
    .bind(submission_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let answers: Vec<SubmissionAnswerDetailDto> = answer_rows
        .into_iter()
        .map(|a| SubmissionAnswerDetailDto {
            question_id: a.get("question_id"),
            question_text: a.get("question_text"),
            question_type: a.get("question_type"),
            max_points: a.get("max_points"),
            chosen_choice_id: a.get("chosen_choice_id"),
            chosen_choice_text: a.get("chosen_choice_text"),
            is_correct: a.get("is_correct"),
            text_answer: a.get("text_answer"),
            points_earned: a.get::<Option<i32>, _>("points_earned").unwrap_or(0),
            teacher_feedback: a.get("teacher_feedback"),
            interaction_config: a.try_get("interaction_config").ok().flatten(),
        })
        .collect();

    // Otomatis tambahkan poin butir soal hasil penilaian guru ke XP siswa
    let total_question_points: i32 = answers.iter().map(|a| a.points_earned.max(0)).sum();
    let earned_xp = if !answers.is_empty() && total_question_points > 0 {
        total_question_points
    } else {
        payload.score.max(0)
    };
    let assignment_title: String = sqlx::query_scalar::<_, String>(
        "SELECT title FROM assignments WHERE id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(assignment_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Tugas".to_string());
    let xp_desc = format!("Poin Tugas: {} ({} Poin)", assignment_title, earned_xp);
    let _ = crate::presentation::gamification::controller::sync_scored_xp_for_student(
        &ctx.pool,
        req_ctx.tenant_id,
        submission.student_id,
        "ASSIGNMENT_SCORE",
        submission_id,
        earned_xp,
        &xp_desc,
    )
    .await;

    crate::presentation::learning::grade_sync::sync_assignment_submission_to_gradebook(
        &ctx.pool,
        req_ctx.tenant_id,
        submission_id,
    )
    .await;

    let mut resp = SubmissionResponse::from(submission);
    if let Some(si) = student_info {
        resp.student_name = si.get("student_name");
        resp.student_nisn = si.get("student_nisn");
    }
    resp.answers = answers;

    Ok(Json(ApiResponse::success(resp, req_ctx.request_id)))
}
