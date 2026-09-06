use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    assessment_rule_response::AssessmentRuleResponse,
    calculate_grade_request::CalculateGradeRequest, configure_rules_request::ConfigureRulesRequest,
    gradebook_entry_response::GradebookEntryResponse,
    gradebook_response::GradeBookResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::assessment::{
    calculate_grade::{CalculateGradeCommand, ComponentScoreInput},
    configure_rules::{ComponentInput, ConfigureRulesCommand},
    get_rules::GetRulesQuery,
};

pub fn assessment_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/rules", post(configure_rules).get(get_rules))
        .route("/calculate", post(calculate_grade))
        .route("/gradebook", get(get_gradebook))
        .route("/gradebook/save", post(save_gradebook))
}

async fn configure_rules(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<ConfigureRulesRequest>,
) -> Result<Json<ApiResponse<AssessmentRuleResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssessmentConfigure).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = ConfigureRulesCommand {
        tenant_id: req_ctx.tenant_id,
        class_id: payload.class_id,
        subject_id: payload.subject_id,
        academic_term_id: payload.academic_term_id,
        minimum_passing_grade: payload.minimum_passing_grade,
        components: payload
            .components
            .into_iter()
            .map(|c| ComponentInput {
                name: c.name,
                component_type: c.component_type,
                weight_percentage: c.weight_percentage,
                is_required: c.is_required,
                order_index: c.order_index,
            })
            .collect(),
    };

    let rule = ctx
        .configure_assessment_rules
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssessmentRuleResponse::from(rule),
        req_ctx.request_id,
    )))
}

async fn get_rules(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<GetRulesParams>,
) -> Result<Json<ApiResponse<AssessmentRuleResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssessmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetRulesQuery {
        class_id: params.class_id,
        subject_id: params.subject_id,
    };

    let rule = ctx
        .get_assessment_rules
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssessmentRuleResponse::from(rule),
        req_ctx.request_id,
    )))
}

#[derive(serde::Deserialize)]
pub struct GetRulesParams {
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

async fn calculate_grade(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CalculateGradeRequest>,
) -> Result<Json<ApiResponse<GradeBookResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssessmentConfigure).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CalculateGradeCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: payload.student_id,
        class_id: payload.class_id,
        subject_id: payload.subject_id,
        academic_year_id: payload.academic_year_id,
        scores: payload
            .scores
            .into_iter()
            .map(|s| ComponentScoreInput {
                component_name: s.component_name,
                source_type: s.source_type,
                raw_score: s.raw_score,
                max_raw_score: s.max_raw_score,
                source_id: s.source_id,
            })
            .collect(),
    };

    let gradebook = ctx
        .calculate_grade
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        GradeBookResponse::from(gradebook),
        req_ctx.request_id,
    )))
}

#[derive(serde::Deserialize)]
pub struct SaveGradebookRequest {
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub academic_year_id: Option<Uuid>,
    pub grades: Vec<StudentGradeInput>,
}

#[derive(serde::Deserialize)]
pub struct StudentGradeInput {
    pub student_id: Uuid,
    pub formatif1: Option<f64>,
    pub formatif2: Option<f64>,
    pub pts: Option<f64>,
    pub pas: Option<f64>,
    pub final_score: Option<f64>,
    pub letter_grade: Option<String>,
    pub passed: Option<bool>,
    pub status: Option<String>,
}

async fn save_gradebook(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SaveGradebookRequest>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssessmentConfigure)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherCreate))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::AcademicManage))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StaffRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to save grades".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    // Resolve subject_id if not directly provided
    let resolved_subject_id = match payload.subject_id {
        Some(sid) => Some(sid),
        None => {
            if let Some(ref sname) = payload.subject_name {
                sqlx::query_scalar!(
                    r#"SELECT id FROM subjects WHERE tenant_id = $1 AND (name ILIKE $2 OR code ILIKE $2) LIMIT 1"#,
                    req_ctx.tenant_id,
                    sname
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            } else {
                sqlx::query_scalar!(
                    r#"SELECT id FROM subjects WHERE tenant_id = $1 LIMIT 1"#,
                    req_ctx.tenant_id
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten()
            }
        }
    };

    let subject_id = match resolved_subject_id {
        Some(sid) => sid,
        None => {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::ResourceNotFound,
                    "Mata pelajaran tidak ditemukan".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    for g in payload.grades {
        // Resolve class_id from enrollment if not provided in payload
        let class_id = match payload.class_id {
            Some(cid) => cid,
            None => {
                let enrolled_cid = sqlx::query_scalar!(
                    r#"SELECT class_id FROM enrollments WHERE student_id = $1 AND (status = 'Active' OR status = 'ACTIVE') LIMIT 1"#,
                    g.student_id
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();

                match enrolled_cid {
                    Some(cid) => cid,
                    None => continue,
                }
            }
        };

        let gb_id = Uuid::new_v4();
        let passed = g.passed.unwrap_or_else(|| g.final_score.map(|s| s >= 75.0).unwrap_or(false));
        let status = g.status.unwrap_or_else(|| "published".to_string());
        let fs_str = g.final_score.map(|s| format!("{:.2}", s));

        // 1. Upsert gradebooks table
        let record = sqlx::query!(
            r#"
            INSERT INTO gradebooks (id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score, letter_grade, passed, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7::TEXT::NUMERIC, $8, $9, $10, NOW(), NOW())
            ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
            SET final_score = COALESCE($7::TEXT::NUMERIC, gradebooks.final_score),
                letter_grade = COALESCE($8, gradebooks.letter_grade),
                passed = $9,
                status = $10,
                updated_at = NOW()
            RETURNING id
            "#,
            gb_id,
            req_ctx.tenant_id,
            g.student_id,
            class_id,
            subject_id,
            payload.academic_year_id,
            fs_str,
            g.letter_grade,
            passed,
            status
        )
        .fetch_one(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;

        let inserted_gb_id = record.id;

        // 2. Clean previous component entries for this student/class/subject
        let _ = sqlx::query!(
            r#"DELETE FROM gradebook_entries WHERE student_id = $1 AND class_id = $2 AND subject_id = $3"#,
            g.student_id,
            class_id,
            subject_id
        )
        .execute(&ctx.pool)
        .await;

        // 3. Insert component scores
        let components: [(&str, &str, Option<f64>, f64); 4] = [
            ("Formatif 1", "assignment", g.formatif1, 20.0),
            ("Formatif 2", "assignment", g.formatif2, 20.0),
            ("PTS (Tengah Semester)", "exam", g.pts, 30.0),
            ("PAS (Akhir Semester)", "exam", g.pas, 30.0),
        ];

        for (comp_name, src_type, raw_opt, weight) in components {
            if let Some(raw) = raw_opt {
                let entry_id = Uuid::new_v4();
                let weighted = raw * (weight / 100.0);
                let raw_str = format!("{:.2}", raw);
                let weighted_str = format!("{:.2}", weighted);
                let weight_str = format!("{:.2}", weight);

                let _ = sqlx::query!(
                    r#"
                    INSERT INTO gradebook_entries (
                        id, tenant_id, student_id, class_id, subject_id,
                        component_name, source_type, raw_score, max_raw_score,
                        weighted_score, weight_percentage, calculated_at, created_at, gradebook_id
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8::TEXT::NUMERIC, 100.0, $9::TEXT::NUMERIC, $10::TEXT::NUMERIC, NOW(), NOW(), $11)
                    "#,
                    entry_id,
                    req_ctx.tenant_id,
                    g.student_id,
                    class_id,
                    subject_id,
                    comp_name,
                    src_type,
                    raw_str,
                    weighted_str,
                    weight_str,
                    inserted_gb_id
                )
                .execute(&ctx.pool)
                .await;
            }
        }
    }

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}

async fn get_gradebook(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<GradebookParams>,
) -> Result<Json<ApiResponse<Vec<GradebookEntryResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssessmentRead)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StudentRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::GuardianRead))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::StaffRead))
        .map_err(|_| {
            ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    // If student_id is not specified in params, check if the current actor is a student or guardian
    let target_student_id = match params.student_id {
        Some(sid) => Some(sid),
        None => {
            if let Some(aid) = actor_id {
                let sid = sqlx::query_scalar!(
                    r#"SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1"#,
                    aid,
                    req_ctx.tenant_id
                )
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();

                if sid.is_some() {
                    sid
                } else {
                    // Check if guardian
                    sqlx::query_scalar!(
                        r#"
                        SELECT s.id
                        FROM guardians g
                        JOIN students s ON s.guardian_id = g.id
                        WHERE g.user_id = $1 AND g.tenant_id = $2
                        LIMIT 1
                        "#,
                        aid,
                        req_ctx.tenant_id
                    )
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten()
                }
            } else {
                None
            }
        }
    };

    let entries = sqlx::query!(
        r#"
        SELECT ge.id, ge.student_id, ge.class_id, ge.subject_id, ge.component_name, ge.source_type,
               ge.raw_score::TEXT as raw_score_text,
               ge.max_raw_score::TEXT as max_raw_score_text,
               ge.weighted_score::TEXT as weighted_score_text,
               ge.weight_percentage::TEXT as weight_percentage_text,
               ge.calculated_at,
               sub.name as subject_name
        FROM gradebook_entries ge
        LEFT JOIN subjects sub ON sub.id = ge.subject_id
        WHERE ge.tenant_id = $1
          AND ($2::uuid IS NULL OR ge.student_id = $2)
          AND ($3::uuid IS NULL OR ge.class_id = $3)
          AND ($4::uuid IS NULL OR ge.subject_id = $4)
        ORDER BY ge.calculated_at DESC
        "#,
        req_ctx.tenant_id,
        target_student_id,
        params.class_id,
        params.subject_id
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    let items = entries
        .into_iter()
        .map(|e| GradebookEntryResponse {
            id: e.id,
            student_id: e.student_id,
            class_id: e.class_id,
            subject_id: e.subject_id,
            subject_name: Some(e.subject_name),
            component_name: e.component_name,
            source_type: e.source_type,
            raw_score: e.raw_score_text.and_then(|s| s.parse::<f64>().ok()),
            max_raw_score: e.max_raw_score_text.and_then(|s| s.parse::<f64>().ok()),
            weighted_score: e.weighted_score_text.and_then(|s| s.parse::<f64>().ok()),
            weight_percentage: e.weight_percentage_text.and_then(|s| s.parse::<f64>().ok()),
            calculated_at: e.calculated_at,
        })
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

#[derive(serde::Deserialize)]
pub struct GradebookParams {
    pub student_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
}
