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

    // --- BATCH OPTIMIZATION: Process all grades in 4 queries instead of 6N queries ---
    if payload.grades.is_empty() {
        return Ok(Json(ApiResponse::success(true, req_ctx.request_id)));
    }

    // Collect all student IDs for batch operations
    let student_ids: Vec<Uuid> = payload.grades.iter().map(|g| g.student_id).collect();

    // Batch resolve class_ids from enrollments (single query)
    let class_id_map: std::collections::HashMap<Uuid, Uuid> = if payload.class_id.is_none() {
        sqlx::query!(
            r#"
            SELECT DISTINCT ON (e.student_id) e.student_id, e.class_id
            FROM enrollments e
            WHERE e.student_id = ANY($1) AND (e.status = 'Active' OR e.status = 'ACTIVE')
            ORDER BY e.student_id
            "#,
            &student_ids
        )
        .fetch_all(&ctx.pool)
        .await
        .ok()
        .map(|rows| {
            rows.into_iter()
                .filter_map(|r| Some((r.student_id, r.class_id)))
                .collect()
        })
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let resolved_class_id = payload.class_id;

    // Prepare batch data for gradebooks
    let mut gb_ids: Vec<Uuid> = Vec::with_capacity(payload.grades.len());
    let mut gb_tenant_ids: Vec<Uuid> = Vec::with_capacity(payload.grades.len());
    let mut gb_student_ids: Vec<Uuid> = Vec::with_capacity(payload.grades.len());
    let mut gb_class_ids: Vec<Uuid> = Vec::with_capacity(payload.grades.len());
    let mut gb_subject_ids: Vec<Uuid> = Vec::with_capacity(payload.grades.len());
    let mut gb_academic_year_ids: Vec<Option<Uuid>> = Vec::with_capacity(payload.grades.len());
    let mut gb_final_scores: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_letter_grades: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_passed_values: Vec<bool> = Vec::with_capacity(payload.grades.len());
    let mut gb_statuses: Vec<String> = Vec::with_capacity(payload.grades.len());

    for g in &payload.grades {
        let class_id = match resolved_class_id {
            Some(cid) => cid,
            None => match class_id_map.get(&g.student_id) {
                Some(cid) => *cid,
                None => continue,
            },
        };

        gb_ids.push(Uuid::new_v4());
        gb_tenant_ids.push(req_ctx.tenant_id);
        gb_student_ids.push(g.student_id);
        gb_class_ids.push(class_id);
        gb_subject_ids.push(subject_id);
        gb_academic_year_ids.push(payload.academic_year_id);
        gb_final_scores.push(g.final_score.map(|s| format!("{:.2}", s)));
        gb_letter_grades.push(g.letter_grade.clone());
        gb_passed_values.push(g.passed.unwrap_or_else(|| g.final_score.map(|s| s >= 75.0).unwrap_or(false)));
        gb_statuses.push(g.status.clone().unwrap_or_else(|| "published".to_string()));
    }

    if gb_ids.is_empty() {
        return Ok(Json(ApiResponse::success(true, req_ctx.request_id)));
    }

    let now = chrono::Utc::now();
    let now_vec = vec![now; gb_ids.len()];

    // Batch upsert gradebooks using UNNEST (single query)
    sqlx::query!(
        r#"
        INSERT INTO gradebooks (id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score, letter_grade, passed, status, created_at, updated_at)
        SELECT
            u.id, u.tenant_id, u.student_id, u.class_id, u.subject_id, u.academic_year_id,
            u.final_score::NUMERIC, u.letter_grade, u.passed, u.status, u.created_at, u.updated_at
        FROM UNNEST(
            $1::uuid[], $2::uuid[], $3::uuid[], $4::uuid[], $5::uuid[],
            $6::uuid[], $7::text[], $8::text[], $9::bool[], $10::text[],
            $11::timestamptz[], $12::timestamptz[]
        ) AS u(id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score, letter_grade, passed, status, created_at, updated_at)
        ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
        SET final_score = COALESCE(EXCLUDED.final_score, gradebooks.final_score),
            letter_grade = COALESCE(EXCLUDED.letter_grade, gradebooks.letter_grade),
            passed = EXCLUDED.passed,
            status = EXCLUDED.status,
            updated_at = NOW()
        "#,
        &gb_ids,
        &gb_tenant_ids,
        &gb_student_ids,
        &gb_class_ids,
        &gb_subject_ids,
        &gb_academic_year_ids as &[Option<Uuid>],
        &gb_final_scores as &[Option<String>],
        &gb_letter_grades as &[Option<String>],
        &gb_passed_values,
        &gb_statuses,
        &now_vec,
        &now_vec
    )
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    // Batch delete all previous component entries (single query)
    let _ = sqlx::query!(
        r#"
        DELETE FROM gradebook_entries
        WHERE student_id = ANY($1) AND class_id = ANY($2) AND subject_id = $3
        "#,
        &gb_student_ids,
        &gb_class_ids,
        subject_id
    )
    .execute(&ctx.pool)
    .await;

    // Prepare batch data for gradebook_entries
    let mut entry_ids: Vec<Uuid> = Vec::new();
    let mut entry_tenant_ids: Vec<Uuid> = Vec::new();
    let mut entry_student_ids: Vec<Uuid> = Vec::new();
    let mut entry_class_ids: Vec<Uuid> = Vec::new();
    let mut entry_subject_ids: Vec<Uuid> = Vec::new();
    let mut entry_component_names: Vec<String> = Vec::new();
    let mut entry_source_types: Vec<String> = Vec::new();
    let mut entry_raw_scores: Vec<String> = Vec::new();
    let mut entry_weighted_scores: Vec<String> = Vec::new();
    let mut entry_weight_percentages: Vec<String> = Vec::new();

    for (i, g) in payload.grades.iter().enumerate() {
        if i >= gb_ids.len() {
            break;
        }

        let components: [(&str, &str, Option<f64>, f64); 4] = [
            ("Formatif 1", "assignment", g.formatif1, 20.0),
            ("Formatif 2", "assignment", g.formatif2, 20.0),
            ("PTS (Tengah Semester)", "exam", g.pts, 30.0),
            ("PAS (Akhir Semester)", "exam", g.pas, 30.0),
        ];

        for (comp_name, src_type, raw_opt, weight) in components {
            if let Some(raw) = raw_opt {
                let weighted = raw * (weight / 100.0);
                entry_ids.push(Uuid::new_v4());
                entry_tenant_ids.push(req_ctx.tenant_id);
                entry_student_ids.push(g.student_id);
                entry_class_ids.push(gb_class_ids[i]);
                entry_subject_ids.push(subject_id);
                entry_component_names.push(comp_name.to_string());
                entry_source_types.push(src_type.to_string());
                entry_raw_scores.push(format!("{:.2}", raw));
                entry_weighted_scores.push(format!("{:.2}", weighted));
                entry_weight_percentages.push(format!("{:.2}", weight));
            }
        }
    }

    // Batch insert all component entries using UNNEST (single query)
    if !entry_ids.is_empty() {
        let entry_now_vec = vec![now; entry_ids.len()];
        sqlx::query!(
            r#"
            INSERT INTO gradebook_entries (
                id, tenant_id, student_id, class_id, subject_id,
                component_name, source_type, raw_score, max_raw_score,
                weighted_score, weight_percentage, calculated_at, created_at
            )
            SELECT
                u.id, u.tenant_id, u.student_id, u.class_id, u.subject_id,
                u.component_name, u.source_type,
                u.raw_score::NUMERIC, 100.0,
                u.weighted_score::NUMERIC, u.weight_percentage::NUMERIC,
                u.calculated_at, u.created_at
            FROM UNNEST(
                $1::uuid[], $2::uuid[], $3::uuid[], $4::uuid[], $5::uuid[],
                $6::text[], $7::text[], $8::text[], $9::text[], $10::text[],
                $11::timestamptz[], $12::timestamptz[]
            ) AS u(id, tenant_id, student_id, class_id, subject_id, component_name, source_type, raw_score, weighted_score, weight_percentage, calculated_at, created_at)
            "#,
            &entry_ids,
            &entry_tenant_ids,
            &entry_student_ids,
            &entry_class_ids,
            &entry_subject_ids,
            &entry_component_names,
            &entry_source_types,
            &entry_raw_scores,
            &entry_weighted_scores,
            &entry_weight_percentages,
            &entry_now_vec,
            &entry_now_vec
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;
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



