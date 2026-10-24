use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    assessment_rule_response::AssessmentRuleResponse,
    calculate_grade_request::CalculateGradeRequest, configure_rules_request::ConfigureRulesRequest,
    gradebook_entry_response::GradebookEntryResponse, gradebook_response::GradeBookResponse,
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
        .route("/gradebook/summary", get(get_gradebook_summary))
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
    pub weight_sumatif: Option<f64>,
    pub weight_ats: Option<f64>,
    pub weight_sas: Option<f64>,
    pub kktp: Option<f64>,
    pub grades: Vec<StudentGradeInput>,
}

#[derive(serde::Deserialize, Clone)]
pub struct StudentGradeInput {
    pub student_id: Uuid,
    pub sumatif_m1: Option<f64>,
    pub sumatif_m2: Option<f64>,
    pub sumatif_m3: Option<f64>,
    pub sumatif_m4: Option<f64>,
    pub sumatif_scores: Option<Vec<f64>>,
    pub ats: Option<f64>,
    #[serde(alias = "pts")]
    pub pts: Option<f64>,
    pub sas: Option<f64>,
    #[serde(alias = "pas")]
    pub pas: Option<f64>,
    pub formatif_notes: Option<String>,
    #[serde(alias = "formatif1")]
    pub formatif1: Option<f64>,
    #[serde(alias = "formatif2")]
    pub formatif2: Option<f64>,
    pub final_score: Option<f64>,
    pub letter_grade: Option<String>,
    pub passed: Option<bool>,
    pub status: Option<String>,
}

#[derive(serde::Serialize)]
pub struct GradebookSummaryRow {
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub subject_name: Option<String>,
    pub sumatif_m1: Option<f64>,
    pub sumatif_m2: Option<f64>,
    pub sumatif_m3: Option<f64>,
    pub sumatif_m4: Option<f64>,
    pub avg_sumatif: Option<f64>,
    pub ats: Option<f64>,
    pub sas: Option<f64>,
    pub final_score: Option<f64>,
    pub letter_grade: Option<String>,
    pub passed: bool,
    pub status: String,
    pub highest_competency_desc: Option<String>,
    pub lowest_competency_desc: Option<String>,
    pub formatif_notes: Option<String>,
}

struct EvaluatedGrade {
    avg_sumatif: Option<f64>,
    ats: Option<f64>,
    sas: Option<f64>,
    final_score: Option<f64>,
    letter_grade: String,
    passed: bool,
    status: String,
    highest_desc: String,
    lowest_desc: String,
    components: Vec<(String, String, f64, f64, f64)>, // (name, src_type, raw, weighted, weight_pct)
}

fn evaluate_kurikulum_merdeka(
    g: &StudentGradeInput,
    kktp: f64,
    custom_w_sumatif: Option<f64>,
    custom_w_ats: Option<f64>,
    custom_w_sas: Option<f64>,
    subject_label: &str,
) -> EvaluatedGrade {
    // 1. Kumpulkan semua nilai Sumatif Lingkup Materi / Modul (Wajib)
    let mut sumatif_list: Vec<(String, f64)> = Vec::new();
    if let Some(s) = g.sumatif_m1.filter(|v| *v >= 0.0) {
        sumatif_list.push(("Sumatif M1".to_string(), s));
    }
    if let Some(s) = g.sumatif_m2.filter(|v| *v >= 0.0) {
        sumatif_list.push(("Sumatif M2".to_string(), s));
    }
    if let Some(s) = g.sumatif_m3.filter(|v| *v >= 0.0) {
        sumatif_list.push(("Sumatif M3".to_string(), s));
    }
    if let Some(s) = g.sumatif_m4.filter(|v| *v >= 0.0) {
        sumatif_list.push(("Sumatif M4".to_string(), s));
    }
    if let Some(ref extra_scores) = g.sumatif_scores {
        for score in extra_scores {
            if *score >= 0.0 {
                let m_idx = sumatif_list.len() + 1;
                sumatif_list.push((format!("Sumatif M{}", m_idx), *score));
            }
        }
    }

    let avg_sumatif = if !sumatif_list.is_empty() {
        let total: f64 = sumatif_list.iter().map(|(_, s)| *s).sum();
        Some(total / sumatif_list.len() as f64)
    } else {
        None
    };

    let ats_val = g.ats.or(g.pts).filter(|v| *v >= 0.0);
    let sas_val = g.sas.or(g.pas).filter(|v| *v >= 0.0);

    // 2. Rumus Pembobotan Resmi Kemendikdasmen RI (Kurikulum Merdeka)
    let mut components = Vec::new();

    let final_score_opt = match (avg_sumatif, ats_val, sas_val) {
        (Some(avg_m), Some(ats), Some(sas)) => {
            // Skema A: (Rata-rata Sumatif Modul x 60%) + (ATS x 20%) + (SAS x 20%)
            let w_m = custom_w_sumatif.unwrap_or(60.0);
            let w_ats = custom_w_ats.unwrap_or(20.0);
            let w_sas = custom_w_sas.unwrap_or(20.0);
            let total_w = w_m + w_ats + w_sas;
            let raw_na = ((avg_m * w_m) + (ats * w_ats) + (sas * w_sas)) / (if total_w > 0.0 { total_w } else { 100.0 });
            
            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, score) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *score, *score * (per_m_w / 100.0), per_m_w));
            }
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * (w_ats / 100.0), w_ats));
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * (w_sas / 100.0), w_sas));
            Some(raw_na)
        }
        (Some(avg_m), Some(ats), None) => {
            // Skema A2 (Jika hanya ATS tanpa SAS): (Modul x 60%) + (ATS x 40%)
            let w_m = custom_w_sumatif.unwrap_or(60.0);
            let w_ats = custom_w_ats.unwrap_or(40.0);
            let total_w = w_m + w_ats;
            let raw_na = ((avg_m * w_m) + (ats * w_ats)) / (if total_w > 0.0 { total_w } else { 100.0 });

            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, score) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *score, *score * (per_m_w / 100.0), per_m_w));
            }
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * (w_ats / 100.0), w_ats));
            Some(raw_na)
        }
        (Some(avg_m), None, Some(sas)) => {
            // Skema A3 (Jika hanya SAS tanpa ATS): (Modul x 60%) + (SAS x 40%)
            let w_m = custom_w_sumatif.unwrap_or(60.0);
            let w_sas = custom_w_sas.unwrap_or(40.0);
            let total_w = w_m + w_sas;
            let raw_na = ((avg_m * w_m) + (sas * w_sas)) / (if total_w > 0.0 { total_w } else { 100.0 });

            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, score) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *score, *score * (per_m_w / 100.0), per_m_w));
            }
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * (w_sas / 100.0), w_sas));
            Some(raw_na)
        }
        (Some(avg_m), None, None) => {
            // Skema B (Hanya Nilai Sumatif Modul - Sangat Direkomendasikan untuk Kesetaraan)
            let per_m_w = 100.0 / sumatif_list.len() as f64;
            for (name, score) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *score, *score * (per_m_w / 100.0), per_m_w));
            }
            Some(avg_m)
        }
        (None, Some(ats), Some(sas)) => {
            let raw_na = (ats + sas) / 2.0;
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * 0.5, 50.0));
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * 0.5, 50.0));
            Some(raw_na)
        }
        (None, Some(ats), None) => {
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats, 100.0));
            Some(ats)
        }
        (None, None, Some(sas)) => {
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas, 100.0));
            Some(sas)
        }
        (None, None, None) => None,
    };

    // Formatif (Assessment FOR & AS Learning): murni kualitatif / refleksi perkembangan, 0% bobot rapor
    if let Some(ref notes) = g.formatif_notes {
        if !notes.trim().is_empty() {
            components.push(("Catatan Formatif".to_string(), "formative_qualitative".to_string(), 100.0, 0.0, 0.0));
        }
    }
    if let Some(f1) = g.formatif1 {
        components.push(("Formatif 1 (Refleksi)".to_string(), "formative_qualitative".to_string(), f1, 0.0, 0.0));
    }
    if let Some(f2) = g.formatif2 {
        components.push(("Formatif 2 (Refleksi)".to_string(), "formative_qualitative".to_string(), f2, 0.0, 0.0));
    }

    // 3. Aturan Pembulatan Angka Rapor Resmi: < 0.5 bulatkan ke bawah, >= 0.5 bulatkan ke atas
    let final_score = final_score_opt.map(|na| na.round());

    let letter_grade = match final_score {
        Some(s) if s >= 88.0 => "A".to_string(),
        Some(s) if s >= 75.0 => "B".to_string(),
        Some(s) if s >= 65.0 => "C".to_string(),
        Some(_) => "D".to_string(),
        None => "-".to_string(),
    };

    let passed = final_score.map(|s| s >= kktp).unwrap_or(false);
    let status = match final_score {
        Some(_) if passed => "Tuntas KKTP".to_string(),
        Some(_) => "Perlu Remedial".to_string(),
        None => "Belum Diinput".to_string(),
    };

    // 4. Fitur Deskripsi Otomatis (Capaian Kompetensi e-Rapor Kemendikdasmen)
    let (highest_desc, lowest_desc) = if !sumatif_list.is_empty() {
        let max_elem = sumatif_list.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        let min_elem = sumatif_list.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();

        let h = if max_elem.1 >= 85.0 {
            format!("Menunjukkan penguasaan yang sangat baik dalam mencapai capaian pembelajaran {}.", max_elem.0)
        } else if max_elem.1 >= 75.0 {
            format!("Menunjukkan penguasaan yang baik dalam mencapai capaian pembelajaran {}.", max_elem.0)
        } else {
            format!("Mencapai penguasaan cukup dalam {}.", max_elem.0)
        };

        let l = if min_elem.1 < kktp {
            format!("Perlu bimbingan dan pendampingan intensif dalam meningkatkan kompetensi {}.", min_elem.0)
        } else if min_elem.1 < 80.0 {
            format!("Perlu penguatan dan latihan berkala untuk memperdalam pemahaman {}.", min_elem.0)
        } else {
            "Menunjukkan konsistensi penguasaan kompetensi yang merata pada seluruh materi modul.".to_string()
        };

        (h, l)
    } else {
        (
            format!("Menunjukkan pemahaman terhadap konsep mata pelajaran {}.", subject_label),
            "Perlu pendampingan belajar secara berkala.".to_string(),
        )
    };

    EvaluatedGrade {
        avg_sumatif,
        ats: ats_val,
        sas: sas_val,
        final_score,
        letter_grade,
        passed,
        status,
        highest_desc,
        lowest_desc,
        components,
    }
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

    if payload.grades.is_empty() {
        return Ok(Json(ApiResponse::success(true, req_ctx.request_id)));
    }

    let subject_label = payload
        .subject_name
        .clone()
        .unwrap_or_else(|| "Mata Pelajaran".to_string());
    let target_kktp = payload.kktp.unwrap_or(75.0);

    // Collect all student IDs for batch operations
    let student_ids: Vec<Uuid> = payload.grades.iter().map(|g| g.student_id).collect();

    // Batch resolve class_ids from enrollments
    let class_id_map: std::collections::HashMap<Uuid, Uuid> = if payload.class_id.is_none() {
        sqlx::query(
            r#"
            SELECT DISTINCT ON (e.student_id) e.student_id, e.class_id
            FROM enrollments e
            WHERE e.student_id = ANY($1) AND (e.status = 'Active' OR e.status = 'ACTIVE')
            ORDER BY e.student_id
            "#,
        )
        .bind(&student_ids)
        .fetch_all(&ctx.pool)
        .await
        .ok()
        .map(|rows| {
            rows.into_iter()
                .filter_map(|r| {
                    let s_id: Option<Uuid> = r.get("student_id");
                    let c_id: Option<Uuid> = r.get("class_id");
                    match (s_id, c_id) {
                        (Some(s), Some(c)) => Some((s, c)),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let resolved_class_id = payload.class_id;

    // Prepare batch data for gradebooks and evaluate Kurikulum Merdeka in Backend
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
    let mut gb_avg_sumatifs: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_ats_scores: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_sas_scores: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_kktp_values: Vec<String> = Vec::with_capacity(payload.grades.len());
    let mut gb_highest_descs: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_lowest_descs: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());
    let mut gb_formatif_notes: Vec<Option<String>> = Vec::with_capacity(payload.grades.len());

    // Component entries
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

    for g in &payload.grades {
        let class_id = match resolved_class_id {
            Some(cid) => cid,
            None => match class_id_map.get(&g.student_id) {
                Some(cid) => *cid,
                None => continue,
            },
        };

        // 100% Backend Execution of Kurikulum Merdeka Formula
        let eval = evaluate_kurikulum_merdeka(
            g,
            target_kktp,
            payload.weight_sumatif,
            payload.weight_ats,
            payload.weight_sas,
            &subject_label,
        );

        gb_ids.push(Uuid::new_v4());
        gb_tenant_ids.push(req_ctx.tenant_id);
        gb_student_ids.push(g.student_id);
        gb_class_ids.push(class_id);
        gb_subject_ids.push(subject_id);
        gb_academic_year_ids.push(payload.academic_year_id);
        gb_final_scores.push(eval.final_score.map(|s| format!("{:.2}", s)));
        gb_letter_grades.push(Some(eval.letter_grade));
        gb_passed_values.push(eval.passed);
        gb_statuses.push(eval.status);
        gb_avg_sumatifs.push(eval.avg_sumatif.map(|s| format!("{:.2}", s)));
        gb_ats_scores.push(eval.ats.map(|s| format!("{:.2}", s)));
        gb_sas_scores.push(eval.sas.map(|s| format!("{:.2}", s)));
        gb_kktp_values.push(format!("{:.2}", target_kktp));
        gb_highest_descs.push(Some(eval.highest_desc));
        gb_lowest_descs.push(Some(eval.lowest_desc));
        gb_formatif_notes.push(g.formatif_notes.clone());

        for (comp_name, src_type, raw, weighted, weight_pct) in eval.components {
            entry_ids.push(Uuid::new_v4());
            entry_tenant_ids.push(req_ctx.tenant_id);
            entry_student_ids.push(g.student_id);
            entry_class_ids.push(class_id);
            entry_subject_ids.push(subject_id);
            entry_component_names.push(comp_name);
            entry_source_types.push(src_type);
            entry_raw_scores.push(format!("{:.2}", raw));
            entry_weighted_scores.push(format!("{:.2}", weighted));
            entry_weight_percentages.push(format!("{:.2}", weight_pct));
        }
    }

    if gb_ids.is_empty() {
        return Ok(Json(ApiResponse::success(true, req_ctx.request_id)));
    }

    let now = chrono::Utc::now();
    let now_vec = vec![now; gb_ids.len()];

    // Batch upsert gradebooks using UNNEST with Kurikulum Merdeka fields
    sqlx::query(
        r#"
        INSERT INTO gradebooks (
            id, tenant_id, student_id, class_id, subject_id, academic_year_id,
            final_score, letter_grade, passed, status,
            avg_sumatif, ats_score, sas_score, kktp,
            highest_competency_desc, lowest_competency_desc, formatif_notes,
            created_at, updated_at
        )
        SELECT
            u.id, u.tenant_id, u.student_id, u.class_id, u.subject_id, u.academic_year_id,
            u.final_score::NUMERIC, u.letter_grade, u.passed, u.status,
            u.avg_sumatif::NUMERIC, u.ats_score::NUMERIC, u.sas_score::NUMERIC, u.kktp::NUMERIC,
            u.highest_desc, u.lowest_desc, u.formatif_notes,
            u.created_at, u.updated_at
        FROM UNNEST(
            $1::uuid[], $2::uuid[], $3::uuid[], $4::uuid[], $5::uuid[], $6::uuid[],
            $7::text[], $8::text[], $9::bool[], $10::text[],
            $11::text[], $12::text[], $13::text[], $14::text[],
            $15::text[], $16::text[], $17::text[],
            $18::timestamptz[], $19::timestamptz[]
        ) AS u(id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score, letter_grade, passed, status, avg_sumatif, ats_score, sas_score, kktp, highest_desc, lowest_desc, formatif_notes, created_at, updated_at)
        ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
        SET final_score = COALESCE(EXCLUDED.final_score, gradebooks.final_score),
            letter_grade = COALESCE(EXCLUDED.letter_grade, gradebooks.letter_grade),
            passed = EXCLUDED.passed,
            status = EXCLUDED.status,
            avg_sumatif = COALESCE(EXCLUDED.avg_sumatif, gradebooks.avg_sumatif),
            ats_score = COALESCE(EXCLUDED.ats_score, gradebooks.ats_score),
            sas_score = COALESCE(EXCLUDED.sas_score, gradebooks.sas_score),
            kktp = COALESCE(EXCLUDED.kktp, gradebooks.kktp),
            highest_competency_desc = COALESCE(EXCLUDED.highest_desc, gradebooks.highest_competency_desc),
            lowest_competency_desc = COALESCE(EXCLUDED.lowest_desc, gradebooks.lowest_competency_desc),
            formatif_notes = COALESCE(EXCLUDED.formatif_notes, gradebooks.formatif_notes),
            updated_at = NOW()
        "#
    )
    .bind(&gb_ids)
    .bind(&gb_tenant_ids)
    .bind(&gb_student_ids)
    .bind(&gb_class_ids)
    .bind(&gb_subject_ids)
    .bind(&gb_academic_year_ids as &[Option<Uuid>])
    .bind(&gb_final_scores as &[Option<String>])
    .bind(&gb_letter_grades as &[Option<String>])
    .bind(&gb_passed_values)
    .bind(&gb_statuses)
    .bind(&gb_avg_sumatifs as &[Option<String>])
    .bind(&gb_ats_scores as &[Option<String>])
    .bind(&gb_sas_scores as &[Option<String>])
    .bind(&gb_kktp_values)
    .bind(&gb_highest_descs as &[Option<String>])
    .bind(&gb_lowest_descs as &[Option<String>])
    .bind(&gb_formatif_notes as &[Option<String>])
    .bind(&now_vec)
    .bind(&now_vec)
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e)
    ), &req_ctx.request_id))?;

    // Batch delete all previous component entries (single query)
    let _ = sqlx::query(
        r#"
        DELETE FROM gradebook_entries
        WHERE student_id = ANY($1) AND class_id = ANY($2) AND subject_id = $3
        "#,
    )
    .bind(&gb_student_ids)
    .bind(&gb_class_ids)
    .bind(subject_id)
    .execute(&ctx.pool)
    .await;

    // Batch insert all component entries using UNNEST
    if !entry_ids.is_empty() {
        let entry_now_vec = vec![now; entry_ids.len()];
        sqlx::query(
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
            "#
        )
        .bind(&entry_ids)
        .bind(&entry_tenant_ids)
        .bind(&entry_student_ids)
        .bind(&entry_class_ids)
        .bind(&entry_subject_ids)
        .bind(&entry_component_names)
        .bind(&entry_source_types)
        .bind(&entry_raw_scores)
        .bind(&entry_weighted_scores)
        .bind(&entry_weight_percentages)
        .bind(&entry_now_vec)
        .bind(&entry_now_vec)
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e)
        ), &req_ctx.request_id))?;
    }

    // FCM update NILAI ke siswa
    {
        let t = format!("🏆 Update Nilai: {}", subject_label);
        let b = "Nilai terbaru Kurikulum Merdeka telah diinput guru. Buka Nilai untuk melihat evaluasi!".to_string();
        let tid = req_ctx.tenant_id;
        let first_student = gb_student_ids.first().copied();
        if let Some(sid) = first_student {
            let user_id: Option<Uuid> =
                sqlx::query_scalar("SELECT user_id FROM students WHERE id = $1")
                    .bind(sid)
                    .fetch_optional(&ctx.pool)
                    .await
                    .ok()
                    .flatten();
            if let Some(uid) = user_id {
                let _ = sqlx::query(
                    "INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'GRADE_UPDATE', 'in_app', 'grade', $5, FALSE, NOW())",
                )
                .bind(tid).bind(uid).bind(&t).bind(&b).bind(sid)
                .execute(&ctx.pool).await;
            }
        }
        let fcm_target = if let Some(cid) = entry_class_ids.first().copied() {
            crate::infrastructure::fcm::FcmTarget::Class(cid)
        } else {
            crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()])
        };
        crate::infrastructure::fcm::trigger_fcm_push_targeted(
            t,
            b,
            crate::infrastructure::fcm::FcmCategory::Grade,
            first_student.unwrap_or_else(Uuid::new_v4),
            fcm_target,
        );
    }

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}

async fn get_gradebook_summary(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<GradebookParams>,
) -> Result<Json<ApiResponse<Vec<GradebookSummaryRow>>>, ApiError> {
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

    let rows = sqlx::query(
        r#"
        SELECT gb.student_id, gb.class_id, gb.subject_id, sub.name as subject_name,
               gb.final_score::TEXT as final_score_text,
               gb.letter_grade,
               COALESCE(gb.passed, false) as passed,
               gb.status,
               gb.avg_sumatif::TEXT as avg_sumatif_text,
               gb.ats_score::TEXT as ats_score_text,
               gb.sas_score::TEXT as sas_score_text,
               gb.highest_competency_desc,
               gb.lowest_competency_desc,
               gb.formatif_notes
        FROM gradebooks gb
        LEFT JOIN subjects sub ON sub.id = gb.subject_id
        WHERE gb.tenant_id = $1
          AND ($2::uuid IS NULL OR gb.class_id = $2)
          AND ($3::uuid IS NULL OR gb.subject_id = $3)
          AND ($4::uuid IS NULL OR gb.student_id = $4)
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(params.class_id)
    .bind(params.subject_id)
    .bind(params.student_id)
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

    // Also fetch individual component entries for modules
    let entries = sqlx::query(
        r#"
        SELECT student_id, component_name, raw_score::TEXT as raw_text
        FROM gradebook_entries
        WHERE tenant_id = $1
          AND ($2::uuid IS NULL OR class_id = $2)
          AND ($3::uuid IS NULL OR subject_id = $3)
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(params.class_id)
    .bind(params.subject_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let mut component_map: std::collections::HashMap<(Uuid, String), f64> = std::collections::HashMap::new();
    for e in entries {
        let sid: Uuid = e.get("student_id");
        let cname: String = e.get("component_name");
        let raw_opt: Option<String> = e.try_get("raw_text").ok();
        if let Some(raw) = raw_opt.and_then(|t| t.parse::<f64>().ok()) {
            component_map.insert((sid, cname), raw);
        }
    }

    let summaries = rows
        .into_iter()
        .map(|r| {
            let sid: Uuid = r.get("student_id");
            let cid: Uuid = r.get("class_id");
            let sub_id: Uuid = r.get("subject_id");
            let sub_name: Option<String> = r.try_get("subject_name").ok();
            let final_text: Option<String> = r.try_get("final_score_text").ok();
            let avg_s_text: Option<String> = r.try_get("avg_sumatif_text").ok();
            let ats_text: Option<String> = r.try_get("ats_score_text").ok();
            let sas_text: Option<String> = r.try_get("sas_score_text").ok();

            let m1 = component_map.get(&(sid, "Sumatif M1".to_string())).copied();
            let m2 = component_map.get(&(sid, "Sumatif M2".to_string())).copied();
            let m3 = component_map.get(&(sid, "Sumatif M3".to_string())).copied();
            let m4 = component_map.get(&(sid, "Sumatif M4".to_string())).copied();

            GradebookSummaryRow {
                student_id: sid,
                class_id: cid,
                subject_id: sub_id,
                subject_name: sub_name,
                sumatif_m1: m1,
                sumatif_m2: m2,
                sumatif_m3: m3,
                sumatif_m4: m4,
                avg_sumatif: avg_s_text.and_then(|t| t.parse::<f64>().ok()),
                ats: ats_text.and_then(|t| t.parse::<f64>().ok()),
                sas: sas_text.and_then(|t| t.parse::<f64>().ok()),
                final_score: final_text.and_then(|t| t.parse::<f64>().ok()),
                letter_grade: r.try_get("letter_grade").ok(),
                passed: r.try_get("passed").unwrap_or(false),
                status: r.try_get("status").unwrap_or_else(|_| "draft".to_string()),
                highest_competency_desc: r.try_get("highest_competency_desc").ok(),
                lowest_competency_desc: r.try_get("lowest_competency_desc").ok(),
                formatif_notes: r.try_get("formatif_notes").ok(),
            }
        })
        .collect();

    Ok(Json(ApiResponse::success(summaries, req_ctx.request_id)))
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
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

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
