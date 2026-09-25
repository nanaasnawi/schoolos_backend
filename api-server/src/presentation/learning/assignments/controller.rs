use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use sqlx::Row;
use uuid::Uuid;

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
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/publish", post(publish))
        .route("/{id}/close", post(close))
        .route("/{id}/archive", post(archive))
        .route("/{id}/submit", post(submit))
        .route("/{id}/submissions", get(get_submissions))
        .route("/{id}/submissions/{submission_id}/grade", post(grade))
}

async fn fetch_assignment_questions(
    pool: &sqlx::PgPool,
    assignment_id: Uuid,
    hide_correct_answers: bool,
) -> Result<Vec<AssignmentQuestionDto>, sqlx::Error> {
    let q_rows = sqlx::query(
        r#"
        SELECT id, question_text, question_type, points, order_index
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
            question_type: q.get("question_type"),
            points: Some(q.get("points")),
            order_index: Some(q.get("order_index")),
            choices,
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
        sqlx::query_scalar!(r#"SELECT id FROM teachers WHERE user_id = $1 LIMIT 1"#, aid)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten()
    } else {
        None
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
        _ => None,
    };

    // Resolve subject_id from description first part (e.g. "Ilmu Pengetahuan Alam dan Sosial (IPAS) • ...")
    let subject_id: Option<Uuid> = if let Some(ref desc) = payload.description {
        let first_part = desc.split('•').next().map(|s| s.trim()).unwrap_or("");
        if !first_part.is_empty() {
            sqlx::query_scalar!(
                r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name = $2 OR name ILIKE $2) LIMIT 1"#,
                req_ctx.tenant_id,
                first_part
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

    let final_teacher_id: Option<Uuid> = if teacher_id.is_some() {
        teacher_id
    } else {
        sqlx::query_scalar!(
            r#"
            SELECT COALESCE(
                (SELECT cs.teacher_id FROM class_schedules cs WHERE cs.class_id = $1 AND cs.subject_id = $2 AND cs.tenant_id = $3 LIMIT 1),
                (SELECT c.homeroom_teacher_id FROM classes c WHERE c.id = $1 AND c.tenant_id = $3 LIMIT 1),
                (SELECT t.id FROM teachers t WHERE t.user_id = $4 AND t.tenant_id = $3 LIMIT 1),
                (SELECT t.id FROM teachers t WHERE t.tenant_id = $3 ORDER BY t.created_at ASC LIMIT 1)
            ) as "teacher_id?"
            "#,
            target_class_id,
            subject_id,
            req_ctx.tenant_id,
            actor_id
        )
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten()
        .flatten()
    };

    let _ = sqlx::query!(
        r#"UPDATE assignments SET class_id = $1, subject_id = $2, teacher_id = $3, created_by = $4, status = 'published', is_active = true WHERE id = $5"#,
        target_class_id,
        subject_id,
        final_teacher_id,
        actor_id,
        assignment.id
    )
    .execute(&ctx.pool)
    .await;

    let (class_name, subject_name, teacher_name) = tokio::join!(
        async {
            if let Some(cid) = target_class_id {
                sqlx::query_scalar!(r#"SELECT name FROM classes WHERE id = $1"#, cid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
        async {
            if let Some(sid) = subject_id {
                sqlx::query_scalar!(r#"SELECT name FROM subjects WHERE id = $1"#, sid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
        async {
            if let Some(tid) = final_teacher_id {
                sqlx::query_scalar!(r#"SELECT full_name FROM teachers WHERE id = $1"#, tid)
                    .fetch_optional(&ctx.pool).await.ok().flatten()
            } else {
                None
            }
        },
    );

    let questions_to_return = if let Some(questions) = payload.questions {
        let mut created_questions = Vec::new();
        for (q_idx, q) in questions.into_iter().enumerate() {
            let question_id = q.id.unwrap_or_else(Uuid::new_v4);
            let order_idx = q.order_index.unwrap_or(q_idx as i32);
            let points = q.points.unwrap_or(10);
            let _ = sqlx::query(
                r#"
                INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(question_id)
            .bind(req_ctx.tenant_id)
            .bind(assignment.id)
            .bind(&q.question_text)
            .bind(&q.question_type)
            .bind(points)
            .bind(order_idx)
            .execute(&ctx.pool)
            .await;

            let mut created_choices = Vec::new();
            for (c_idx, c) in q.choices.into_iter().enumerate() {
                let choice_id = c.id.unwrap_or_else(Uuid::new_v4);
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
                .execute(&ctx.pool)
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
            });
        }
        created_questions
    } else {
        Vec::new()
    };

    let mut resp = AssignmentResponse::from(assignment);
    resp.status = "published".to_string();
    resp.class_id = target_class_id;
    resp.class_name = class_name;
    resp.subject_name = subject_name;
    resp.teacher_name = teacher_name;
    resp.questions = questions_to_return;

    // Kirim push TUGAS baru ke siswa rombel target (atau semua siswa bila tanpa kelas).
    {
        let t = format!("📝 Tugas Baru: {}", assignment_title_for_notif);
        let b = match (&resp.class_name, &resp.teacher_name) {
            (Some(c), Some(g)) => format!("{} memberi tugas baru untuk kelas {}. Kerjakan sebelum tenggat!", g, c),
            (Some(c), None) => format!("Ada tugas baru untuk kelas {}. Kerjakan sebelum tenggat!", c),
            _ => format!("Ada tugas baru: {}. Kerjakan sebelum tenggat!", assignment_title_for_notif),
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
            WHERE s.tenant_id = $3
              AND ($5::uuid IS NULL OR en.class_id = $5)
              AND (en.status = 'Active' OR en.status = 'ACTIVE')
            "#,
        )
        .bind(&title_c)
        .bind(&body_c)
        .bind(tid)
        .bind(assignment_id_for_notif)
        .bind(cid)
        .execute(&ctx.pool)
        .await;
        crate::infrastructure::fcm::trigger_fcm_push_categorized(
            t, b,
            crate::infrastructure::fcm::FcmCategory::Assignment,
            assignment_id_for_notif,
        );
    }

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}


async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AssignmentResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

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
        || crate::authorization_helpers::AuthorizationScope::resolve_teacher_id(&ctx.pool, req_ctx.tenant_id, actor_id.unwrap_or_default()).await.ok().flatten().is_some();
    let is_parent = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n.contains("wali") || n.contains("parent") || n.contains("guardian") || n.contains("ortu")
            })
        })
        .unwrap_or(false);
    let is_student = !is_parent && !is_teacher && req_ctx
        .actor
        .as_ref()
        .map(|a| a.roles.iter().any(|r| r.name == "Siswa"))
        .unwrap_or(false);

    // Self-healing: Ensure assignments with class_id have published status and teacher_id resolved
    let _ = sqlx::query!(
        r#"
        UPDATE assignments
        SET status = 'published'
        WHERE tenant_id = $1 
          AND status = 'draft' 
          AND class_id IS NOT NULL 
          AND deleted_at IS NULL
        "#,
        req_ctx.tenant_id
    )
    .execute(&ctx.pool)
    .await;

    let _ = sqlx::query!(
        r#"
        UPDATE assignments a
        SET teacher_id = COALESCE(
            (SELECT cs.teacher_id FROM class_schedules cs WHERE cs.class_id = a.class_id AND cs.subject_id = a.subject_id AND cs.tenant_id = a.tenant_id LIMIT 1),
            (SELECT c.homeroom_teacher_id FROM classes c WHERE c.id = a.class_id AND c.tenant_id = a.tenant_id LIMIT 1),
            (SELECT t.id FROM teachers t WHERE t.user_id = a.created_by AND t.tenant_id = a.tenant_id LIMIT 1),
            (SELECT t.id FROM teachers t WHERE t.tenant_id = a.tenant_id ORDER BY t.created_at ASC LIMIT 1)
        )
        WHERE a.tenant_id = $1 AND a.teacher_id IS NULL AND a.deleted_at IS NULL
        "#,
        req_ctx.tenant_id
    )
    .execute(&ctx.pool)
    .await;

    let items: Vec<AssignmentResponse> = if is_teacher {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                CASE WHEN a.status = 'draft' AND a.class_id IS NOT NULL THEN 'published' ELSE a.status END as status,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
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
                ) as teacher_name
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND (
                  a.created_by = $2 
                  OR a.teacher_id IN (SELECT id FROM teachers WHERE user_id = $2 AND tenant_id = $1)
                  OR a.class_id IN (
                      SELECT cs.class_id FROM class_schedules cs 
                      JOIN teachers t ON t.id = cs.teacher_id 
                      WHERE t.user_id = $2 AND cs.tenant_id = $1
                  )
                  OR a.class_id IN (
                      SELECT c.id FROM classes c 
                      JOIN teachers t ON t.id = c.homeroom_teacher_id 
                      WHERE t.user_id = $2 AND c.tenant_id = $1
                  )
              )
            ORDER BY a.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
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
                questions: Vec::new(),
            })
            .collect()
    } else if is_student || is_parent {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                CASE WHEN a.status = 'draft' AND a.class_id IS NOT NULL THEN 'published' ELSE a.status END as status,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
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
                ) as teacher_name
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 
              AND a.deleted_at IS NULL
              AND (
                  a.class_id IS NULL
                  OR a.class_id IN (
                      SELECT en.class_id 
                      FROM students s
                      JOIN enrollments en ON en.student_id = s.id
                      WHERE s.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                      UNION
                      SELECT en.class_id 
                      FROM guardians g
                      JOIN students s ON s.guardian_id = g.id
                      JOIN enrollments en ON en.student_id = s.id
                      WHERE g.user_id = $2 AND (en.status = 'Active' OR en.status = 'ACTIVE')
                  )
              )
            ORDER BY a.created_at DESC
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(actor_id)
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
                questions: Vec::new(),
            })
            .collect()
    } else {
        let rows = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.lesson_id, a.title, a.description, a.instructions,
                a.max_score, a.due_at, a.assignment_type,
                CASE WHEN a.status = 'draft' AND a.class_id IS NOT NULL THEN 'published' ELSE a.status END as status,
                a.is_active,
                a.created_at, a.updated_at,
                a.class_id,
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
                ) as teacher_name
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN subjects sub ON sub.id = a.subject_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            WHERE a.tenant_id = $1 AND a.deleted_at IS NULL
            ORDER BY a.created_at DESC
            "#,
        )
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
                questions: Vec::new(),
            })
            .collect()
    };

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
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let _ = sqlx::query(
        r#"
        UPDATE assignments
        SET status = 'published'
        WHERE id = $1 AND tenant_id = $2 AND status = 'draft' AND class_id IS NOT NULL AND deleted_at IS NULL
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
            CASE WHEN a.status = 'draft' AND a.class_id IS NOT NULL THEN 'published' ELSE a.status END as status,
            a.is_active,
            a.created_at, a.updated_at,
            a.class_id, a.teacher_id, a.created_by,
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
            ) as teacher_name
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
                &ctx.pool,
                &req_ctx,
                tenant_id,
                class_id,
                teacher_id,
                created_by,
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
                teacher_name: r.get("teacher_name"),
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

    if let Some(questions) = payload.questions {
        let _ = sqlx::query("DELETE FROM assignment_questions WHERE assignment_id = $1")
            .bind(id)
            .execute(&ctx.pool)
            .await;

        for (q_idx, q) in questions.into_iter().enumerate() {
            let question_id = q.id.unwrap_or_else(Uuid::new_v4);
            let order_idx = q.order_index.unwrap_or(q_idx as i32);
            let points = q.points.unwrap_or(10);
            let _ = sqlx::query(
                r#"
                INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(question_id)
            .bind(req_ctx.tenant_id)
            .bind(id)
            .bind(&q.question_text)
            .bind(&q.question_type)
            .bind(points)
            .bind(order_idx)
            .execute(&ctx.pool)
            .await;

            for (c_idx, c) in q.choices.into_iter().enumerate() {
                let choice_id = c.id.unwrap_or_else(Uuid::new_v4);
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
                .execute(&ctx.pool)
                .await;
            }
        }
    }

    let questions = fetch_assignment_questions(&ctx.pool, id, false).await.unwrap_or_default();
    let mut resp = AssignmentResponse::from(assignment);
    resp.questions = questions;

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
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
        let t = format!("📝 Tugas Dipublish: {}", assignment.title);
        crate::infrastructure::fcm::trigger_fcm_push_categorized(
            t,
            "Tugas baru sudah dipublish. Buka aplikasi untuk mengerjakan.".to_string(),
            crate::infrastructure::fcm::FcmCategory::Assignment,
            assignment.id,
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

async fn submit(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubmitAssignmentRequest>,
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

    let assignment = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM assignments WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL"
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

    let mut auto_score: i32 = 0;
    let mut has_essay = false;
    let mut answered_any = false;

    if let Some(answers) = payload.answers {
        for ans in answers {
            answered_any = true;
            let q_info = sqlx::query(
                r#"
                SELECT q.question_type, q.points, c.is_correct
                FROM assignment_questions q
                LEFT JOIN assignment_question_choices c ON c.id = $2 AND c.question_id = q.id
                WHERE q.id = $1
                "#,
            )
            .bind(ans.question_id)
            .bind(ans.chosen_choice_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            let points_earned: i32 = if let Some(ref info) = q_info {
                let q_type: String = info.get("question_type");
                let pts: i32 = info.get("points");
                let is_corr: Option<bool> = info.get("is_correct");
                if q_type == "MULTIPLE_CHOICE" {
                    if is_corr.unwrap_or(false) {
                        pts
                    } else {
                        0
                    }
                } else {
                    has_essay = true;
                    0
                }
            } else {
                0
            };

            auto_score += points_earned;

            let _ = sqlx::query(
                r#"
                INSERT INTO assignment_submission_answers (
                    submission_id, question_id, chosen_choice_id, text_answer, points_earned
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (submission_id, question_id) DO UPDATE SET
                    chosen_choice_id = EXCLUDED.chosen_choice_id,
                    text_answer = EXCLUDED.text_answer,
                    points_earned = EXCLUDED.points_earned
                "#,
            )
            .bind(submission.id)
            .bind(ans.question_id)
            .bind(ans.chosen_choice_id)
            .bind(&ans.text_answer)
            .bind(points_earned)
            .execute(&ctx.pool)
            .await;
        }
    }

    let final_submission = if answered_any && !has_essay && payload.file_url.is_none() {
        let _ = sqlx::query(
            r#"
            UPDATE assignment_submissions
            SET score = $1, status = 'Graded', graded_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(auto_score)
        .bind(submission.id)
        .execute(&ctx.pool)
        .await;

        let mut s = submission;
        s.score = Some(auto_score);
        s.status = "Graded".to_string();
        s
    } else {
        submission
    };

    let mut resp = SubmissionResponse::from(final_submission);

    let answer_rows = sqlx::query(
        r#"
        SELECT 
            ans.question_id,
            q.question_text,
            q.question_type,
            q.points as max_points,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            c.is_correct as is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback
        FROM assignment_submission_answers ans
        JOIN assignment_questions q ON q.id = ans.question_id
        LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.submission_id = $1
        ORDER BY q.order_index ASC, ans.created_at ASC
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
        })
        .collect();

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
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
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let asg_row = sqlx::query(
        "SELECT tenant_id, class_id, teacher_id, created_by FROM assignments WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
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

    let rows = sqlx::query(
        r#"
        SELECT 
            sub.id, sub.tenant_id, sub.assignment_id, sub.student_id,
            sub.content, sub.file_url, sub.submitted_at, sub.status,
            sub.score, sub.feedback, sub.graded_at, sub.graded_by,
            COALESCE(s.full_name, u.full_name, 'Siswa') as student_name,
            COALESCE(s.nisn, u.username, '-') as student_nisn
        FROM assignment_submissions sub
        LEFT JOIN students s ON s.id = sub.student_id OR s.user_id = sub.student_id
        LEFT JOIN users u ON u.id = sub.student_id
        WHERE sub.assignment_id = $1
        ORDER BY sub.submitted_at DESC
        "#,
    )
    .bind(id)
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

    let sub_ids: Vec<Uuid> = rows.iter().map(|r| r.get::<Uuid, _>("id")).collect();

    let answer_rows = if !sub_ids.is_empty() {
        sqlx::query(
            r#"
            SELECT 
                ans.submission_id,
                ans.question_id,
                q.question_text,
                q.question_type,
                q.points as max_points,
                ans.chosen_choice_id,
                c.choice_text as chosen_choice_text,
                c.is_correct as is_correct,
                ans.text_answer,
                ans.points_earned,
                ans.teacher_feedback
            FROM assignment_submission_answers ans
            JOIN assignment_questions q ON q.id = ans.question_id
            LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
            WHERE ans.submission_id = ANY($1)
            ORDER BY q.order_index ASC, ans.created_at ASC
            "#,
        )
        .bind(&sub_ids)
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    let items = rows
        .into_iter()
        .map(|r| {
            let sub_id: Uuid = r.get("id");
            let answers = answer_rows
                .iter()
                .filter(|a| a.get::<Uuid, _>("submission_id") == sub_id)
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
                status: r.get("status"),
                score: r.get("score"),
                feedback: r.get("feedback"),
                graded_at: r.get("graded_at"),
                graded_by: r.get("graded_by"),
                student_name: r.get("student_name"),
                student_nisn: r.get("student_nisn"),
                answers,
            }
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn grade(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, submission_id)): Path<(Uuid, Uuid)>,
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

    if let Some(ref answer_grades) = payload.answer_grades {
        for ag in answer_grades {
            let _ = sqlx::query(
                r#"
                UPDATE assignment_submission_answers
                SET points_earned = $1, teacher_feedback = $2, graded_at = NOW(), graded_by = $3
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
            q.question_text,
            q.question_type,
            q.points as max_points,
            ans.chosen_choice_id,
            c.choice_text as chosen_choice_text,
            c.is_correct as is_correct,
            ans.text_answer,
            ans.points_earned,
            ans.teacher_feedback
        FROM assignment_submission_answers ans
        JOIN assignment_questions q ON q.id = ans.question_id
        LEFT JOIN assignment_question_choices c ON c.id = ans.chosen_choice_id
        WHERE ans.submission_id = $1
        ORDER BY q.order_index ASC, ans.created_at ASC
        "#,
    )
    .bind(submission_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let answers = answer_rows
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
        })
        .collect();

    let mut resp = SubmissionResponse::from(submission);
    if let Some(si) = student_info {
        resp.student_name = si.get("student_name");
        resp.student_nisn = si.get("student_nisn");
    }
    resp.answers = answers;

    Ok(Json(ApiResponse::success(
        resp,
        req_ctx.request_id,
    )))
}

