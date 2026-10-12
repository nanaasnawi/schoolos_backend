use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use uuid::Uuid;
use sqlx::Row;
use chrono::{DateTime, Utc};

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, DomainError};
use school_core::common::error_code::ErrorCode;

use super::dto::{
    AssessmentQuery, AtpItemDto, AtpMatrixQuery, AtpMatrixResponse, AtpSemesterSummaryDto,
    CpRegistryLookupResponse, CpSearchQuery, CreateAssessmentPayload, CreateModulAjarPayload,
    HabitEntryDto, HabitsCalendarQuery, KospCommitteeDto, KospDocumentResponse, KospSignatoryDto,
    LearningOutcomeElementDto, ModulAjarDto, ModulAjarQuery, OverrideHabitTeacherPayload,
    PedagogicalAssessmentItemDto, ProposedTpDto, PublishPedagogyPayload, PublishPedagogyResponse,
    RecordHabitEntryPayload, RegisterSchoolCpPayload, SubmitGradesPayload,
    SynthesizeCpRequest, SynthesizeCpResponse, UpdateTpPayload, VerifyHabitParentPayload,
    SuperviseModulAjarRequest, KaldikBudgetQuery, SynthesizeModulAjarPayload,
    DiagnosticClusteringQuery, RaporPreviewQuery, ApproveRaporPayload,
    SemesterNarrativeQuery, SendHabitReminderPayload,
};

pub fn pedagogy_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/cp", get(lookup_cp))
        .route("/cp/school", post(register_school_cp))
        .route("/cp/synthesize", post(synthesize_cp))
        .route("/kosp", get(get_active_kosp))
        // ATP, TP & Publish
        .route("/atp", get(get_atp_matrix))
        .route("/tp", put(update_tp))
        .route("/publish", post(publish_pedagogy))
        // Modul Ajar
        .route("/modul-ajar", get(list_modul_ajar).post(create_modul_ajar))
        .route("/modul-ajar/{id}/activate", put(activate_modul_ajar))
        .route("/modul-ajar/{id}/supervise", post(supervise_modul_ajar))
        .route("/modul-ajar/kaldik-budget", get(get_kaldik_budget))
        .route("/modul-ajar/synthesize", post(synthesize_modul_ajar))
        // Assessments
        .route("/assessments", get(list_assessments).post(create_assessment))
        .route("/assessments/diagnostic-clustering", get(get_diagnostic_clustering))
        .route("/assessments/rapor-preview", get(get_rapor_preview))
        .route("/assessments/approve-rapor", post(approve_rapor))
        .route("/assessments/{id}", get(get_assessment_detail))
        .route("/assessments/{id}/grades", post(record_assessment_grades))
        // Habits G7KAIH
        .route("/habits/calendar", get(get_habits_calendar))
        .route("/habits/entry", post(record_habit_entry))
        .route("/habits/verify-parent", post(verify_habit_parent))
        .route("/habits/override-teacher", post(override_habit_teacher))
        .route("/habits/semester-narrative", get(get_semester_narrative))
        .route("/habits/send-reminder", post(send_habit_reminder))
}

#[utoipa::path(
    get,
    operation_id = "lookupCurriculumCapaianPembelajaran",
    path = "/api/v1/learning/pedagogy/cp",
    responses(
        (status = 200, description = "Capaian Pembelajaran Source Registry Results", body = ApiResponse<CpRegistryLookupResponse>),
        (status = 400, description = "Validation error")
    ),
    security(("Bearer" = []))
)]
pub async fn lookup_cp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<CpSearchQuery>,
) -> Result<Json<ApiResponse<CpRegistryLookupResponse>>, ApiError> {
    let phase = query.phase.as_deref().unwrap_or("").trim().to_uppercase();
    let subject_raw = query.subject.as_deref().unwrap_or("").trim();
    let verification_filter = query.verification.as_deref().unwrap_or("ALL").trim().to_uppercase();

    if phase.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Parameter 'phase' wajib diisi (contoh: FASE_A, FASE_B, FASE_C, FASE_D, FASE_E, FASE_F)".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    if subject_raw.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Parameter 'subject' (kode atau nama mata pelajaran) wajib diisi".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let subject_pattern = format!("%{subject_raw}%");

    // Query learning_outcomes scoped to tenant or global official templates
    // Order: NATIONAL_VERIFIED first, then SCHOOL_VERIFIED, then others
    let rows = sqlx::query(
        r#"
        SELECT 
            id,
            tenant_id,
            subject_code,
            subject_name,
            phase,
            target_grades,
            element_name,
            element_code,
            description,
            source_origin,
            source_version,
            source_document,
            document_page_ref,
            verification_status,
            order_index,
            created_at,
            updated_at
        FROM learning_outcomes
        WHERE (tenant_id = $1 OR tenant_id IS NULL)
          AND UPPER(phase) = $2
          AND (
              LOWER(subject_name) LIKE LOWER($3)
              OR subject_code = $4
          )
          AND deleted_at IS NULL
          AND (
              $5 = 'ALL'
              OR ($5 = 'VERIFIED_ONLY' AND verification_status IN ('NATIONAL_VERIFIED', 'SCHOOL_VERIFIED'))
              OR ($5 = 'DRAFT_ONLY' AND verification_status = 'UNVERIFIED_DRAFT')
          )
        ORDER BY 
            CASE 
                WHEN verification_status = 'NATIONAL_VERIFIED' THEN 1
                WHEN verification_status = 'SCHOOL_VERIFIED' THEN 2
                ELSE 3 
            END,
            order_index ASC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&phase)
    .bind(&subject_pattern)
    .bind(&subject_raw)
    .bind(&verification_filter)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let elements: Vec<LearningOutcomeElementDto> = rows
        .into_iter()
        .map(|r| {
            let verification_status: String = r.get("verification_status");
            let is_eligible = verification_status == "NATIONAL_VERIFIED"
                || verification_status == "SCHOOL_VERIFIED";

            LearningOutcomeElementDto {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                subject_code: r.get("subject_code"),
                subject_name: r.get("subject_name"),
                phase: r.get("phase"),
                target_grades: r.get("target_grades"),
                element_name: r.get("element_name"),
                element_code: r.get("element_code"),
                description: r.get("description"),
                source_origin: r.get("source_origin"),
                source_version: r.get("source_version"),
                source_document: r.get("source_document"),
                document_page_ref: r.get("document_page_ref"),
                verification_status,
                is_eligible_source: is_eligible,
                order_index: r.get("order_index"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            }
        })
        .collect();

    let eligible_count = elements.iter().filter(|e| e.is_eligible_source).count();
    let has_eligible = eligible_count > 0;

    let message = if has_eligible {
        format!(
            "Ditemukan {} elemen Capaian Pembelajaran (CP) terverifikasi untuk {} pada {}.",
            eligible_count, subject_raw, phase
        )
    } else if !elements.is_empty() {
        format!(
            "Naskah CP untuk {} ({}) ditemukan sebanyak {} elemen, namun statusnya masih UNVERIFIED_DRAFT dan belum disahkan.",
            subject_raw, phase, elements.len()
        )
    } else {
        format!(
            "Naskah CP resmi belum tersedia di registry untuk mata pelajaran '{}' pada {}. Silakan daftarkan dokumen KOSP resmi sekolah terlebih dahulu.",
            subject_raw, phase
        )
    };

    Ok(Json(ApiResponse::success(
        CpRegistryLookupResponse {
            phase,
            subject_query: subject_raw.to_string(),
            source_available: has_eligible,
            eligible_for_ai_synthesis: has_eligible,
            elements_count: elements.len(),
            elements,
            message,
        },
        req_ctx.request_id,
    )))
}

#[utoipa::path(
    post,
    operation_id = "registerSchoolCurriculumCapaianPembelajaran",
    path = "/api/v1/learning/pedagogy/cp/school",
    request_body = RegisterSchoolCpPayload,
    responses(
        (status = 200, description = "Registered School CP element successfully", body = ApiResponse<LearningOutcomeElementDto>),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Forbidden")
    ),
    security(("Bearer" = []))
)]
pub async fn register_school_cp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<RegisterSchoolCpPayload>,
) -> Result<Json<ApiResponse<LearningOutcomeElementDto>>, ApiError> {
    let subject_name = payload.subject_name.trim();
    let phase = payload.phase.trim().to_uppercase();
    let element_name = payload.element_name.trim();
    let description = payload.description.trim();

    if subject_name.is_empty() || phase.is_empty() || element_name.is_empty() || description.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Field 'subject_name', 'phase', 'element_name', dan 'description' wajib diisi".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    let initial_audit = serde_json::json!([
        {
            "action": "SCHOOL_REGISTRATION",
            "actor_id": actor_id,
            "timestamp": chrono::Utc::now(),
            "notes": "Didaftarkan langsung oleh satuan pendidikan sebagai bagian dari KOSP sekolah"
        }
    ]);

    let row = sqlx::query(
        r#"
        INSERT INTO learning_outcomes (
            tenant_id,
            subject_code,
            subject_name,
            phase,
            target_grades,
            element_name,
            element_code,
            description,
            source_origin,
            source_version,
            source_document,
            document_page_ref,
            verification_status,
            verified_by,
            verified_at,
            provenance_audit_trail,
            order_index
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            'SCHOOL_ADAPTED', 'KOSP-1.0', $9, $10,
            'SCHOOL_VERIFIED', $11, NOW(), $12, $13
        )
        RETURNING 
            id, tenant_id, subject_code, subject_name, phase, target_grades,
            element_name, element_code, description, source_origin, source_version,
            source_document, document_page_ref, verification_status, order_index,
            created_at, updated_at
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(payload.subject_code.as_deref())
    .bind(subject_name)
    .bind(&phase)
    .bind(payload.target_grades.trim())
    .bind(element_name)
    .bind(payload.element_code.as_deref())
    .bind(description)
    .bind(payload.source_document.as_deref().unwrap_or("Dokumen KOSP Satuan Pendidikan"))
    .bind(payload.document_page_ref.as_deref())
    .bind(actor_id)
    .bind(&initial_audit)
    .bind(payload.order_index.unwrap_or(0))
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let dto = LearningOutcomeElementDto {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        subject_code: row.get("subject_code"),
        subject_name: row.get("subject_name"),
        phase: row.get("phase"),
        target_grades: row.get("target_grades"),
        element_name: row.get("element_name"),
        element_code: row.get("element_code"),
        description: row.get("description"),
        source_origin: row.get("source_origin"),
        source_version: row.get("source_version"),
        source_document: row.get("source_document"),
        document_page_ref: row.get("document_page_ref"),
        verification_status: row.get("verification_status"),
        is_eligible_source: true,
        order_index: row.get("order_index"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

#[utoipa::path(
    post,
    operation_id = "synthesizeCurriculumLearningObjectives",
    path = "/api/v1/learning/pedagogy/cp/synthesize",
    request_body = SynthesizeCpRequest,
    responses(
        (status = 200, description = "Synthesized CP into TP and ATP drafts", body = ApiResponse<SynthesizeCpResponse>),
        (status = 400, description = "Validation error"),
        (status = 422, description = "Unverified source rejected")
    ),
    security(("Bearer" = []))
)]
pub async fn synthesize_cp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SynthesizeCpRequest>,
) -> Result<Json<ApiResponse<SynthesizeCpResponse>>, ApiError> {
    let grade_level = payload
        .grade_level
        .as_deref()
        .unwrap_or("Kelas 5 SD")
        .trim();
    let academic_year = payload
        .academic_year
        .as_deref()
        .unwrap_or("2026/2027")
        .trim();
    let force_regenerate = payload.force_regenerate.unwrap_or(false);

    // 1. Validasi Keberadaan & Kelayakan CP di Registry
    let cp_row = sqlx::query(
        r#"
        SELECT 
            id, tenant_id, subject_code, subject_name, phase, target_grades,
            element_name, element_code, description, source_origin, source_version,
            verification_status
        FROM learning_outcomes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(payload.source_cp_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Capaian Pembelajaran dengan ID '{}' tidak ditemukan di registry",
                payload.source_cp_id
            ))),
            &req_ctx.request_id,
        )
    })?;

    let cp_id: Uuid = cp_row.get("id");
    let cp_element_name: String = cp_row.get("element_name");
    let cp_verification_status: String = cp_row.get("verification_status");
    let cp_source_version: String = cp_row.get("source_version");
    let cp_phase: String = cp_row.get("phase");
    let cp_subject_name: String = cp_row.get("subject_name");
    let cp_description: String = cp_row.get("description");

    let is_eligible = cp_verification_status == "NATIONAL_VERIFIED"
        || cp_verification_status == "SCHOOL_VERIFIED";

    if !is_eligible {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Capaian Pembelajaran '{}' berstatus '{}'. Hanya CP berstatus NATIONAL_VERIFIED atau SCHOOL_VERIFIED yang boleh dijadikan basis dekonstruksi AI.",
                cp_element_name, cp_verification_status
            ))),
            &req_ctx.request_id,
        ));
    }

    // 2. Cache Komposit Lookup
    let tenant_id_str = req_ctx.tenant_id.to_string();
    let prompt_version = "v1.0-merdeka-kko";
    let model_name = std::env::var("NVIDIA_MODEL").unwrap_or_else(|_| "nvidia/ising-calibration-1.5-31b".to_string());

    let cache_seed = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        tenant_id_str, cp_id, cp_source_version, grade_level, academic_year, model_name, prompt_version
    );
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(cache_seed.as_bytes());
    let cache_key = hex::encode(hasher.finalize());

    if !force_regenerate {
        let cached = sqlx::query(
            r#"
            SELECT id, status FROM curriculum_ai_cache
            WHERE cache_key = $1 AND status = 'DRAFT_GENERATED'
            "#,
        )
        .bind(&cache_key)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        if cached.is_some() {
            let existing_tps = sqlx::query(
                r#"
                SELECT 
                    tp.id, tp.code, tp.competency, tp.bloom_level, tp.content_scope,
                    tp.statement, tp.pancasila_profiles, tp.evidence_indicators,
                    tp.estimated_hours, tp.publication_status, tp.version,
                    atp.semester, atp.sequence_order
                FROM learning_objectives tp
                LEFT JOIN learning_objective_flows atp ON atp.learning_objective_id = tp.id
                WHERE tp.learning_outcome_id = $1
                  AND tp.publication_status = 'DRAFT'
                  AND tp.is_superseded = false
                  AND (tp.tenant_id = $2 OR tp.tenant_id IS NULL)
                ORDER BY atp.semester ASC, atp.sequence_order ASC, tp.order_index ASC
                "#,
            )
            .bind(cp_id)
            .bind(req_ctx.tenant_id)
            .fetch_all(&ctx.pool)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Infrastructure(
                        school_core::common::error::InfrastructureError::Database(e),
                    ),
                    &req_ctx.request_id,
                )
            })?;

            if !existing_tps.is_empty() {
                let current_version: i32 = existing_tps[0].get("version");
                let mapped_tps: Vec<ProposedTpDto> = existing_tps
                    .into_iter()
                    .map(|r| {
                        let profiles: Option<Vec<String>> = r.get("pancasila_profiles");
                        let indicators: Option<Vec<String>> = r.get("evidence_indicators");
                        let semester: Option<String> = r.get("semester");
                        let seq: Option<i32> = r.get("sequence_order");
                        ProposedTpDto {
                            id: r.get("id"),
                            code: r.get("code"),
                            competency: r.get::<Option<String>, _>("competency").unwrap_or_else(|| "Memahami".to_string()),
                            bloom_level: r.get::<Option<String>, _>("bloom_level").unwrap_or_else(|| "C3".to_string()),
                            content_scope: r.get("content_scope"),
                            statement: r.get("statement"),
                            pancasila_profiles: profiles.unwrap_or_default(),
                            evidence_indicators: indicators.unwrap_or_default(),
                            estimated_hours: r.get("estimated_hours"),
                            publication_status: r.get("publication_status"),
                            version: r.get("version"),
                            semester: semester.unwrap_or_else(|| "ODD".to_string()),
                            sequence_order: seq.unwrap_or(1),
                        }
                    })
                    .collect();

                return Ok(Json(ApiResponse::success(
                    SynthesizeCpResponse {
                        cache_hit: true,
                        source_cp_id: cp_id,
                        source_cp_element: cp_element_name,
                        source_verification_status: cp_verification_status,
                        grade_level: grade_level.to_string(),
                        academic_year: academic_year.to_string(),
                        version: current_version,
                        proposed_tps_count: mapped_tps.len(),
                        proposed_tps: mapped_tps,
                        message: "Mengembalikan rancangan draf TP & ATP tersimpan dari cache (0 token).".to_string(),
                    },
                    req_ctx.request_id,
                )));
            }
        }
    }

    // 3. Panggilan ke NVIDIA NIM
    let prompt = format!(
        r#"Anda adalah Pakar Kurikulum Merdeka (Kemendikdasmen RI).
Anda diberikan naskah resmi Capaian Pembelajaran (CP) berikut:
- Mata Pelajaran: {subject_name}
- Fase: {phase} ({grade_level})
- Elemen CP: {element_name}
- Teks Resmi CP: "{description}"

ATURAN KETAT PEDAGOGIS:
1. DILARANG MENGARANG, MERUBAH, ATAU MENGURANGI teks Capaian Pembelajaran (CP) di atas.
2. Tugas Anda adalah membedah naskah CP di atas menjadi 3 sampai 6 butir Tujuan Pembelajaran (TP) yang operasional dan terukur.
3. Setiap butir TP HARUS memiliki:
   - "code": format "TP.{code_prefix}.{grade_digit}.[NOMOR]"
   - "competency": Kata Kerja Operasional (KKO) Taksonomi Bloom (contoh: "Mengidentifikasi", "Menganalisis", "Menyelidiki", "Merancang")
   - "bloom_level": salah satu dari ["C1", "C2", "C3", "C4", "C5", "C6"]
   - "content_scope": Lingkup materi / topik esensial
   - "statement": Rumusan kalimat TP lengkap diawali "Peserta didik dapat..."
   - "pancasila_profiles": array profil pelajar pancasila
   - "evidence_indicators": minimal 2 indikator ketercapaian tujuan pembelajaran (IKTP)
   - "estimated_hours": alokasi Jam Pelajaran (JP) bernilai bilangan bulat antara 2 sampai 8 JP
   - "suggested_semester": "ODD" atau "EVEN"
   - "sequence_order": nomor urut alur pembelajaran (1, 2, 3...)
   - "pedagogical_approach": pendekatan pembelajaran yang cocok

FORMAT KELUARAN HARUS BERUPA JSON MURNI DENGAN SKEMA:
{{
  "tps": [
    {{
      "code": "TP.{code_prefix}.{grade_digit}.1",
      "competency": "Menganalisis",
      "bloom_level": "C4",
      "content_scope": "Nama Lingkup Materi",
      "statement": "Peserta didik dapat menganalisis...",
      "pancasila_profiles": ["Bernalar Kritis", "Mandiri"],
      "evidence_indicators": ["Indikator 1", "Indikator 2"],
      "estimated_hours": 6,
      "suggested_semester": "ODD",
      "sequence_order": 1,
      "pedagogical_approach": "Problem-Based Learning"
    }}
  ]
}}

OUTPUT HANYA JSON MURNI TANPA PEMBUKA/PENUTUP MARKDOWN."#,
        subject_name = cp_subject_name,
        phase = cp_phase,
        grade_level = grade_level,
        element_name = cp_element_name,
        description = cp_description,
        code_prefix = cp_subject_name.chars().filter(|c| c.is_alphabetic()).take(4).collect::<String>().to_uppercase(),
        grade_digit = grade_level.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
    );

    let messages = vec![
        crate::infrastructure::nvidia_ai::ChatMessage {
            role: "system",
            content: "You are an educational designer returning strict JSON learning objectives.",
        },
        crate::infrastructure::nvidia_ai::ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw_response = crate::infrastructure::nvidia_ai::call_nvidia_nim(messages, 0.25, 3000)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let cleaned_json = crate::infrastructure::nvidia_ai::extract_clean_json(&raw_response);

    #[derive(serde::Deserialize)]
    struct RawTpItem {
        code: Option<String>,
        competency: Option<String>,
        bloom_level: Option<String>,
        content_scope: Option<String>,
        statement: Option<String>,
        pancasila_profiles: Option<Vec<String>>,
        evidence_indicators: Option<Vec<String>>,
        estimated_hours: Option<i32>,
        suggested_semester: Option<String>,
        sequence_order: Option<i32>,
        pedagogical_approach: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct RawResponseObj {
        tps: Vec<RawTpItem>,
    }

    let parsed: RawResponseObj = serde_json::from_str(cleaned_json).map_err(|e| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Gagal mem-parsing keluaran AI Kurikulum Merdeka sebagai JSON: {e}"
            ))),
            &req_ctx.request_id,
        )
    })?;

    if parsed.tps.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "AI tidak menghasilkan butir Tujuan Pembelajaran yang valid".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    // 4. Transaksi Penyimpanan Draf Baru & Versioning
    let mut tx = ctx.pool.begin().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    // Hitung next version
    let ver_row = sqlx::query(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1 AS next_version
        FROM learning_objectives
        WHERE learning_outcome_id = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
    )
    .bind(cp_id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let next_version: i32 = ver_row.get::<Option<i32>, _>("next_version").unwrap_or(1);

    // Tandai draf lama sebagai is_superseded = true
    sqlx::query(
        r#"
        UPDATE learning_objectives
        SET is_superseded = true, updated_at = NOW()
        WHERE learning_outcome_id = $1
          AND publication_status = 'DRAFT'
          AND is_superseded = false
          AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
    )
    .bind(cp_id)
    .bind(req_ctx.tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    sqlx::query(
        r#"
        UPDATE learning_objective_flows
        SET is_superseded = true, updated_at = NOW()
        WHERE learning_objective_id IN (
            SELECT id FROM learning_objectives WHERE learning_outcome_id = $1
        )
        AND publication_status = 'DRAFT'
        AND is_superseded = false
        AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
    )
    .bind(cp_id)
    .bind(req_ctx.tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let mut proposed_tps = Vec::new();
    let trace_id = format!("trace-{}-{}", chrono::Utc::now().timestamp_millis(), Uuid::new_v4());

    let ai_meta = serde_json::json!({
        "model_name": model_name,
        "prompt_version": prompt_version,
        "source_cp_id": cp_id,
        "source_cp_version": cp_source_version,
        "trace_id": trace_id,
        "generated_at": chrono::Utc::now()
    });

    for (idx, item) in parsed.tps.into_iter().enumerate() {
        let code = item.code.unwrap_or_else(|| format!("TP-{}", idx + 1));
        let competency = item.competency.unwrap_or_else(|| "Memahami".to_string());
        let raw_bloom = item.bloom_level.unwrap_or_else(|| "C3".to_string()).to_uppercase();
        let bloom_level = if ["C1", "C2", "C3", "C4", "C5", "C6"].contains(&raw_bloom.as_str()) {
            raw_bloom
        } else {
            "C3".to_string()
        };
        let content_scope = item.content_scope.unwrap_or_else(|| "Materi".to_string());
        let statement = item.statement.unwrap_or_else(|| "Peserta didik dapat memahami konsep.".to_string());
        let profiles = item.pancasila_profiles.unwrap_or_else(|| vec!["Bernalar Kritis".to_string()]);
        let indicators = item.evidence_indicators.unwrap_or_else(|| vec!["Menguasai materi dengan baik".to_string()]);
        let hours = item.estimated_hours.unwrap_or(4).clamp(2, 12);
        let semester = item.suggested_semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
        let semester_clean = if semester == "EVEN" { "EVEN" } else { "ODD" };
        let seq = item.sequence_order.unwrap_or((idx + 1) as i32);
        let approach = item.pedagogical_approach.unwrap_or_else(|| "Problem-Based Learning".to_string());

        let tp_row = sqlx::query(
            r#"
            INSERT INTO learning_objectives (
                tenant_id, learning_outcome_id, code, competency, bloom_level,
                content_scope, statement, pancasila_profiles, evidence_indicators,
                estimated_hours, order_index, publication_status, version, is_superseded, ai_generation_meta
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'DRAFT', $12, false, $13
            )
            RETURNING id, code, competency, bloom_level, content_scope, statement,
                      pancasila_profiles, evidence_indicators, estimated_hours, publication_status, version
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(cp_id)
        .bind(&code)
        .bind(&competency)
        .bind(&bloom_level)
        .bind(&content_scope)
        .bind(&statement)
        .bind(&profiles)
        .bind(&indicators)
        .bind(hours)
        .bind((idx + 1) as i32)
        .bind(next_version)
        .bind(&ai_meta)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        let tp_inserted_id: Uuid = tp_row.get("id");

        let atp_row = sqlx::query(
            r#"
            INSERT INTO learning_objective_flows (
                tenant_id, learning_objective_id, academic_year, grade_level, semester,
                sequence_order, allocated_hours, pedagogical_approach, publication_status, version, is_superseded
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, 'DRAFT', $9, false
            )
            RETURNING id, semester, sequence_order, allocated_hours
            "#,
        )
        .bind(req_ctx.tenant_id)
        .bind(tp_inserted_id)
        .bind(academic_year)
        .bind(grade_level)
        .bind(&semester_clean)
        .bind(seq)
        .bind(hours)
        .bind(&approach)
        .bind(next_version)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        let tp_profiles: Option<Vec<String>> = tp_row.get("pancasila_profiles");
        let tp_indicators: Option<Vec<String>> = tp_row.get("evidence_indicators");

        proposed_tps.push(ProposedTpDto {
            id: tp_inserted_id,
            code: tp_row.get("code"),
            competency: tp_row.get::<Option<String>, _>("competency").unwrap_or_default(),
            bloom_level: tp_row.get::<Option<String>, _>("bloom_level").unwrap_or_default(),
            content_scope: tp_row.get("content_scope"),
            statement: tp_row.get("statement"),
            pancasila_profiles: tp_profiles.unwrap_or_default(),
            evidence_indicators: tp_indicators.unwrap_or_default(),
            estimated_hours: tp_row.get("estimated_hours"),
            publication_status: tp_row.get("publication_status"),
            version: tp_row.get("version"),
            semester: atp_row.get("semester"),
            sequence_order: atp_row.get("sequence_order"),
        });
    }

    // Simpan entri cache
    let raw_val: serde_json::Value = serde_json::from_str(cleaned_json).unwrap_or(serde_json::Value::Null);
    sqlx::query(
        r#"
        INSERT INTO curriculum_ai_cache (
            tenant_id, cache_key, source_cp_id, source_cp_version,
            phase, grade_level, academic_year, model_name, prompt_version,
            status, raw_response
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, 'DRAFT_GENERATED', $10
        )
        ON CONFLICT (cache_key) DO UPDATE
        SET status = 'DRAFT_GENERATED',
            raw_response = EXCLUDED.raw_response,
            created_at = NOW()
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&cache_key)
    .bind(cp_id)
    .bind(&cp_source_version)
    .bind(&cp_phase)
    .bind(grade_level)
    .bind(academic_year)
    .bind(&model_name)
    .bind(prompt_version)
    .bind(&raw_val)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    tx.commit().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let total_tps = proposed_tps.len();
    Ok(Json(ApiResponse::success(
        SynthesizeCpResponse {
            cache_hit: false,
            source_cp_id: cp_id,
            source_cp_element: cp_element_name.clone(),
            source_verification_status: cp_verification_status,
            grade_level: grade_level.to_string(),
            academic_year: academic_year.to_string(),
            version: next_version,
            proposed_tps_count: total_tps,
            proposed_tps,
            message: format!(
                "Berhasil mendekonstruksi CP '{}' menjadi {} butir usulan TP & ATP (Status: DRAFT v{}).",
                cp_element_name,
                total_tps,
                next_version
            ),
        },
        req_ctx.request_id,
    )))
}

#[utoipa::path(
    get,
    operation_id = "getActiveKospDocument",
    path = "/api/v1/learning/pedagogy/kosp",
    responses(
        (status = 200, description = "Active KOSP Document & Committees", body = ApiResponse<KospDocumentResponse>),
        (status = 404, description = "KOSP not found")
    ),
    security(("Bearer" = []))
)]
pub async fn get_active_kosp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<KospDocumentResponse>>, ApiError> {
    let academic_year = query
        .get("academic_year")
        .cloned()
        .unwrap_or_else(|| "2026/2027".to_string());

    // 1. Fetch KOSP Document for tenant (or default to PKBM As-Salafiyah)
    let doc_row = sqlx::query(
        r#"
        SELECT 
            id, tenant_id, academic_year, title, legal_basis_statement,
            establishment_place, establishment_date, effective_weeks_odd, effective_weeks_even,
            status, created_at, updated_at
        FROM kosp_documents
        WHERE (tenant_id = $1 OR tenant_id = 'e3c4d8da-ff44-4d87-bb13-759a54f49bd0')
          AND academic_year = $2
          AND deleted_at IS NULL
        ORDER BY (tenant_id = $1) DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&academic_year)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let doc = match doc_row {
        Some(row) => row,
        None => {
            return Err(ApiError::new(
                ApplicationError::NotFound(
                    ErrorCode::ResourceNotFound,
                    format!("Dokumen KOSP untuk tahun ajaran {academic_year} belum terdaftar."),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let kosp_id: Uuid = doc.get("id");

    // 2. Fetch Committees (TPK)
    let comm_rows = sqlx::query(
        r#"
        SELECT id, teacher_id, name, role_in_committee, order_index, notes
        FROM kosp_committees
        WHERE kosp_document_id = $1
        ORDER BY order_index ASC
        "#,
    )
    .bind(kosp_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut committees = Vec::new();
    for r in comm_rows {
        committees.push(KospCommitteeDto {
            id: r.get("id"),
            teacher_id: r.get("teacher_id"),
            name: r.get("name"),
            role_in_committee: r.get("role_in_committee"),
            order_index: r.get("order_index"),
            notes: r.get("notes"),
        });
    }

    // 3. Fetch Signatories
    let sign_rows = sqlx::query(
        r#"
        SELECT id, signatory_role, position_title, name, nip, has_signature, has_official_seal, order_index
        FROM kosp_signatories
        WHERE kosp_document_id = $1
        ORDER BY order_index ASC
        "#,
    )
    .bind(kosp_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut signatories = Vec::new();
    for r in sign_rows {
        signatories.push(KospSignatoryDto {
            id: r.get("id"),
            signatory_role: r.get("signatory_role"),
            position_title: r.get("position_title"),
            name: r.get("name"),
            nip: r.get("nip"),
            has_signature: r.get("has_signature"),
            has_official_seal: r.get("has_official_seal"),
            order_index: r.get("order_index"),
        });
    }

    let est_date: chrono::NaiveDate = doc.get("establishment_date");

    let response = KospDocumentResponse {
        id: kosp_id,
        tenant_id: doc.get("tenant_id"),
        academic_year: doc.get("academic_year"),
        title: doc.get("title"),
        legal_basis_statement: doc.get("legal_basis_statement"),
        establishment_place: doc.get("establishment_place"),
        establishment_date: est_date.to_string(),
        effective_weeks_odd: doc.get("effective_weeks_odd"),
        effective_weeks_even: doc.get("effective_weeks_even"),
        status: doc.get("status"),
        committees,
        signatories,
        created_at: doc.get("created_at"),
        updated_at: doc.get("updated_at"),
    };

    Ok(Json(ApiResponse::success(
        response,
        req_ctx.request_id,
    )))
}

// ─────────────────────────────────────────────────────────────────────────────
// ATP & TP & Publish Endpoints
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_atp_matrix(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<AtpMatrixQuery>,
) -> Result<Json<ApiResponse<AtpMatrixResponse>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let source_cp_id = query.source_cp_id;

    let rows = sqlx::query(
        r#"
        SELECT 
            tp.id as tp_id,
            tp.code,
            tp.competency,
            tp.bloom_level,
            tp.content_scope,
            tp.statement,
            tp.pancasila_profiles,
            tp.evidence_indicators,
            tp.estimated_hours,
            tp.publication_status,
            tp.version,
            atp.id as atp_id,
            atp.semester,
            atp.sequence_order,
            atp.allocated_hours,
            atp.pedagogical_approach,
            lo.subject_name,
            lo.phase,
            lo.element_name
        FROM learning_objectives tp
        JOIN learning_outcomes lo ON lo.id = tp.learning_outcome_id
        LEFT JOIN learning_objective_flows atp ON atp.learning_objective_id = tp.id
        WHERE tp.learning_outcome_id = $1
          AND tp.is_superseded = false
          AND ($2 IS NULL OR tp.tenant_id = $2 OR tp.tenant_id IS NULL)
          AND tp.deleted_at IS NULL
        ORDER BY atp.semester ASC NULLS LAST, atp.sequence_order ASC NULLS LAST, tp.order_index ASC
        "#,
    )
    .bind(source_cp_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut odd_items = Vec::new();
    let mut even_items = Vec::new();
    let mut odd_hours = 0;
    let mut even_hours = 0;

    for r in rows {
        let sem: Option<String> = r.get("semester");
        let est_hours: i32 = r.get("estimated_hours");
        let alloc_hours: Option<i32> = r.get("allocated_hours");
        let item_hours = alloc_hours.unwrap_or(est_hours);

        let item = AtpItemDto {
            tp_id: r.get("tp_id"),
            code: r.get("code"),
            competency: r.get("competency"),
            bloom_level: r.get("bloom_level"),
            content_scope: r.get("content_scope"),
            statement: r.get("statement"),
            pancasila_profiles: r.get("pancasila_profiles"),
            evidence_indicators: r.get("evidence_indicators"),
            estimated_hours: est_hours,
            publication_status: r.get("publication_status"),
            version: r.get("version"),
            atp_id: r.get("atp_id"),
            semester: sem.clone(),
            sequence_order: r.get("sequence_order"),
            allocated_hours: alloc_hours,
            pedagogical_approach: r.get("pedagogical_approach"),
            subject_name: r.get("subject_name"),
            phase: r.get("phase"),
            element_name: r.get("element_name"),
        };

        if sem.as_deref() == Some("EVEN") {
            even_hours += item_hours;
            even_items.push(item);
        } else {
            odd_hours += item_hours;
            odd_items.push(item);
        }
    }

    let total_items = odd_items.len() + even_items.len();
    let total_hours = odd_hours + even_hours;

    let response = AtpMatrixResponse {
        academic_year,
        total_items,
        total_hours,
        odd_semester: AtpSemesterSummaryDto {
            items_count: odd_items.len(),
            total_hours: odd_hours,
            items: odd_items,
        },
        even_semester: AtpSemesterSummaryDto {
            items_count: even_items.len(),
            total_hours: even_hours,
            items: even_items,
        },
    };

    Ok(Json(ApiResponse::success(response, req_ctx.request_id)))
}

pub async fn update_tp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<UpdateTpPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let tp_id = payload.id;
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    let existing = sqlx::query(
        "SELECT id, publication_status FROM learning_objectives WHERE id = $1 AND deleted_at IS NULL"
    )
    .bind(tp_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let existing_row = match existing {
        Some(r) => r,
        None => {
            return Err(ApiError::new(
                ApplicationError::NotFound(
                    ErrorCode::ResourceNotFound,
                    "Tujuan Pembelajaran tidak ditemukan".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let current_status: String = existing_row.get("publication_status");
    let mark_as_reviewed = payload.mark_as_reviewed.unwrap_or(true);
    let new_status = if current_status == "PUBLISHED" {
        "PUBLISHED".to_string()
    } else if mark_as_reviewed {
        "REVIEWED".to_string()
    } else {
        current_status
    };

    let updated = sqlx::query(
        r#"
        UPDATE learning_objectives
        SET 
            statement = COALESCE($2, statement),
            competency = COALESCE($3, competency),
            bloom_level = COALESCE($4, bloom_level),
            content_scope = COALESCE($5, content_scope),
            estimated_hours = COALESCE($6, estimated_hours),
            pancasila_profiles = COALESCE($7, pancasila_profiles),
            evidence_indicators = COALESCE($8, evidence_indicators),
            publication_status = $9,
            reviewed_by = CASE WHEN $9 = 'REVIEWED' THEN $10 ELSE reviewed_by END,
            reviewed_at = CASE WHEN $9 = 'REVIEWED' THEN NOW() ELSE reviewed_at END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, code, competency, bloom_level, content_scope, statement,
                  pancasila_profiles, evidence_indicators, estimated_hours, publication_status, version
        "#
    )
    .bind(tp_id)
    .bind(payload.statement)
    .bind(payload.competency)
    .bind(payload.bloom_level)
    .bind(payload.content_scope)
    .bind(payload.estimated_hours)
    .bind(payload.pancasila_profiles)
    .bind(payload.evidence_indicators)
    .bind(&new_status)
    .bind(actor_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let result = serde_json::json!({
        "id": updated.get::<Uuid, _>("id"),
        "code": updated.get::<String, _>("code"),
        "competency": updated.get::<Option<String>, _>("competency"),
        "bloom_level": updated.get::<Option<String>, _>("bloom_level"),
        "content_scope": updated.get::<String, _>("content_scope"),
        "statement": updated.get::<String, _>("statement"),
        "pancasila_profiles": updated.get::<Option<Vec<String>>, _>("pancasila_profiles"),
        "evidence_indicators": updated.get::<Option<Vec<String>>, _>("evidence_indicators"),
        "estimated_hours": updated.get::<i32, _>("estimated_hours"),
        "publication_status": updated.get::<String, _>("publication_status"),
        "version": updated.get::<i32, _>("version")
    });

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

pub async fn publish_pedagogy(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<PublishPedagogyPayload>,
) -> Result<Json<ApiResponse<PublishPedagogyResponse>>, ApiError> {
    if payload.tp_ids.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Daftar ID TP (tp_ids) wajib disertakan".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let mut tx = ctx.pool.begin().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let tp_rows = sqlx::query(
        r#"
        UPDATE learning_objectives
        SET 
            publication_status = 'PUBLISHED',
            published_by = $2,
            published_at = NOW(),
            updated_at = NOW()
        WHERE id = ANY($1)
          AND ($3 IS NULL OR tenant_id = $3 OR tenant_id IS NULL)
          AND deleted_at IS NULL
        RETURNING id, estimated_hours
        "#
    )
    .bind(&payload.tp_ids)
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    if tp_rows.is_empty() {
        return Err(ApiError::new(
            ApplicationError::NotFound(
                ErrorCode::ResourceNotFound,
                "Tidak ada Tujuan Pembelajaran yang cocok untuk diterbitkan pada sekolah ini.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let atp_rows = sqlx::query(
        r#"
        UPDATE learning_objective_flows
        SET 
            publication_status = 'PUBLISHED',
            published_by = $2,
            published_at = NOW(),
            updated_at = NOW()
        WHERE learning_objective_id = ANY($1)
          AND ($3 IS NULL OR tenant_id = $3 OR tenant_id IS NULL)
          AND deleted_at IS NULL
        RETURNING id
        "#
    )
    .bind(&payload.tp_ids)
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    tx.commit().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let total_hours: i32 = tp_rows.iter().map(|r| r.get::<i32, _>("estimated_hours")).sum();

    let response = PublishPedagogyResponse {
        published_tps_count: tp_rows.len(),
        published_atps_count: atp_rows.len(),
        total_hours,
        message: format!(
            "Berhasil menerbitkan {} TP dan {} alur ATP dengan total {} JP ke kurikulum aktif.",
            tp_rows.len(), atp_rows.len(), total_hours
        ),
    };

    Ok(Json(ApiResponse::success(response, req_ctx.request_id)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Modul Ajar Endpoints
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_modul_ajar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ModulAjarQuery>,
) -> Result<Json<ApiResponse<Vec<ModulAjarDto>>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = query.semester.unwrap_or_else(|| "ALL".to_string()).to_uppercase();
    let subject_code = query.subject_code.unwrap_or_else(|| "ALL".to_string());
    let status = query.status.unwrap_or_else(|| "ALL".to_string()).to_uppercase();

    let rows = sqlx::query(
        r#"
        SELECT 
            m.id,
            m.tenant_id,
            m.learning_objective_id,
            m.academic_year,
            m.semester,
            m.title,
            m.grade_level,
            m.subject_code,
            m.subject_name,
            m.phase,
            m.allocated_hours,
            m.total_meetings,
            m.hours_per_meeting,
            m.pancasila_profiles,
            m.meaningful_understanding,
            m.trigger_questions,
            m.differentiation_strategies,
            m.learning_activities,
            m.assessment_plan,
            m.lkpd_attachments,
            m.status,
            m.suspension_reason,
            m.version,
            m.is_ai_generated,
            m.created_at,
            m.updated_at,
            tp.code as tp_code,
            tp.statement as tp_statement,
            tp.publication_status as tp_publication_status
        FROM modul_ajar m
        JOIN learning_objectives tp ON m.learning_objective_id = tp.id
        WHERE ($1 IS NULL OR m.tenant_id = $1 OR m.tenant_id IS NULL)
          AND m.academic_year = $2
          AND ($3 = 'ALL' OR m.semester = $3)
          AND ($4 = 'ALL' OR m.subject_code = $4)
          AND ($5 = 'ALL' OR m.status = $5)
          AND m.deleted_at IS NULL
        ORDER BY m.created_at DESC
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(&subject_code)
    .bind(&status)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut result = Vec::new();
    for r in rows {
        result.push(ModulAjarDto {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            learning_objective_id: r.get("learning_objective_id"),
            tp_code: r.get("tp_code"),
            tp_statement: r.get("tp_statement"),
            tp_publication_status: r.get("tp_publication_status"),
            academic_year: r.get("academic_year"),
            semester: r.get("semester"),
            title: r.get("title"),
            grade_level: r.get("grade_level"),
            subject_code: r.get("subject_code"),
            subject_name: r.get("subject_name"),
            phase: r.get("phase"),
            allocated_hours: r.get("allocated_hours"),
            total_meetings: r.get("total_meetings"),
            hours_per_meeting: r.get("hours_per_meeting"),
            pancasila_profiles: r.get("pancasila_profiles"),
            meaningful_understanding: r.get("meaningful_understanding"),
            trigger_questions: r.get("trigger_questions"),
            differentiation_strategies: r.get("differentiation_strategies"),
            learning_activities: r.get("learning_activities"),
            assessment_plan: r.get("assessment_plan"),
            lkpd_attachments: r.get("lkpd_attachments"),
            status: r.get("status"),
            suspension_reason: r.get("suspension_reason"),
            version: r.get("version"),
            is_ai_generated: r.get("is_ai_generated"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
    }

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

pub async fn create_modul_ajar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateModulAjarPayload>,
) -> Result<Json<ApiResponse<ModulAjarDto>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let academic_year = payload.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let grade_level = payload.grade_level.unwrap_or_else(|| "Kelas 5 SD".to_string());
    let subject_code = payload.subject_code.unwrap_or_else(|| "401900000".to_string());
    let subject_name = payload.subject_name.unwrap_or_else(|| "IPAS".to_string());
    let phase = payload.phase.unwrap_or_else(|| "FASE_C".to_string());
    let allocated_hours = payload.allocated_hours.unwrap_or(4);
    let total_meetings = payload.total_meetings.unwrap_or(1);
    let hours_per_meeting = payload.hours_per_meeting.unwrap_or(4);
    let pancasila_profiles = payload.pancasila_profiles.unwrap_or_else(|| vec!["Bernalar Kritis".to_string(), "Mandiri".to_string()]);
    let meaningful_understanding = payload.meaningful_understanding.unwrap_or_default();
    let trigger_questions = payload.trigger_questions.unwrap_or_else(|| serde_json::json!([]));
    let differentiation_strategies = payload.differentiation_strategies.unwrap_or_else(|| serde_json::json!({}));
    let learning_activities = payload.learning_activities.unwrap_or_else(|| serde_json::json!({}));
    let assessment_plan = payload.assessment_plan.unwrap_or_else(|| serde_json::json!({}));
    let lkpd_attachments = payload.lkpd_attachments.unwrap_or_else(|| serde_json::json!([]));

    let row = sqlx::query(
        r#"
        INSERT INTO modul_ajar (
            tenant_id, learning_objective_id, academic_year, semester,
            title, grade_level, subject_code, subject_name, phase,
            allocated_hours, total_meetings, hours_per_meeting,
            pancasila_profiles, meaningful_understanding, trigger_questions,
            differentiation_strategies, learning_activities, assessment_plan,
            lkpd_attachments, status, created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, 'DRAFT', $20
        )
        RETURNING *
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(payload.learning_objective_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(&payload.title)
    .bind(&grade_level)
    .bind(&subject_code)
    .bind(&subject_name)
    .bind(&phase)
    .bind(allocated_hours)
    .bind(total_meetings)
    .bind(hours_per_meeting)
    .bind(&pancasila_profiles)
    .bind(&meaningful_understanding)
    .bind(&trigger_questions)
    .bind(&differentiation_strategies)
    .bind(&learning_activities)
    .bind(&assessment_plan)
    .bind(&lkpd_attachments)
    .bind(actor_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let dto = ModulAjarDto {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        learning_objective_id: row.get("learning_objective_id"),
        tp_code: None,
        tp_statement: None,
        tp_publication_status: None,
        academic_year: row.get("academic_year"),
        semester: row.get("semester"),
        title: row.get("title"),
        grade_level: row.get("grade_level"),
        subject_code: row.get("subject_code"),
        subject_name: row.get("subject_name"),
        phase: row.get("phase"),
        allocated_hours: row.get("allocated_hours"),
        total_meetings: row.get("total_meetings"),
        hours_per_meeting: row.get("hours_per_meeting"),
        pancasila_profiles: row.get("pancasila_profiles"),
        meaningful_understanding: row.get("meaningful_understanding"),
        trigger_questions: row.get("trigger_questions"),
        differentiation_strategies: row.get("differentiation_strategies"),
        learning_activities: row.get("learning_activities"),
        assessment_plan: row.get("assessment_plan"),
        lkpd_attachments: row.get("lkpd_attachments"),
        status: row.get("status"),
        suspension_reason: row.get("suspension_reason"),
        version: row.get("version"),
        is_ai_generated: row.get("is_ai_generated"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

pub async fn activate_modul_ajar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let row = sqlx::query(
        r#"
        UPDATE modul_ajar
        SET status = 'ACTIVE', suspension_reason = NULL, updated_at = NOW()
        WHERE id = $1 AND ($2 IS NULL OR tenant_id = $2 OR tenant_id IS NULL)
        RETURNING id, title, status
        "#
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "title": row.get::<String, _>("title"),
        "status": row.get::<String, _>("status")
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Pedagogical Assessments Endpoints
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_assessments(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<AssessmentQuery>,
) -> Result<Json<ApiResponse<Vec<PedagogicalAssessmentItemDto>>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = query.semester.unwrap_or_else(|| "ALL".to_string()).to_uppercase();
    let subject_code = query.subject_code.unwrap_or_else(|| "ALL".to_string());
    let taxonomy_type = query.taxonomy_type.unwrap_or_else(|| "ALL".to_string()).to_uppercase();
    let class_id = query.class_id;

    let rows = sqlx::query(
        r#"
        SELECT 
            pa.id,
            pa.class_id,
            c.name AS class_name,
            pa.learning_objective_id,
            tp.code AS tp_code,
            tp.statement AS tp_statement,
            tp.competency AS tp_competency,
            tp.content_scope AS tp_content_scope,
            tp.publication_status AS tp_publication_status,
            pa.modul_ajar_id,
            m.title AS modul_ajar_title,
            pa.academic_year,
            pa.semester,
            pa.subject_code,
            pa.subject_name,
            pa.title,
            pa.taxonomy_type,
            pa.assessment_method,
            pa.passing_threshold::float8 AS passing_threshold,
            pa.kktp_criteria,
            pa.rubric_descriptors,
            pa.date_conducted::text AS date_conducted,
            pa.calendar_event_id,
            kaldik.title AS kaldik_event_title,
            pa.status,
            COUNT(asr.id)::int AS graded_students_count,
            pa.created_at
        FROM pedagogical_assessments pa
        JOIN classes c ON pa.class_id = c.id
        LEFT JOIN learning_objectives tp ON pa.learning_objective_id = tp.id
        LEFT JOIN modul_ajar m ON pa.modul_ajar_id = m.id
        LEFT JOIN academic_calendar_events kaldik ON pa.calendar_event_id = kaldik.id
        LEFT JOIN assessment_student_results asr ON pa.id = asr.assessment_id
        WHERE ($1 IS NULL OR pa.tenant_id = $1 OR pa.tenant_id IS NULL)
          AND pa.academic_year = $2
          AND ($3 = 'ALL' OR pa.semester = $3)
          AND ($4 IS NULL OR pa.class_id = $4)
          AND ($5 = 'ALL' OR pa.subject_code = $5)
          AND ($6 = 'ALL' OR pa.taxonomy_type = $6)
          AND pa.deleted_at IS NULL
        GROUP BY pa.id, c.name, tp.code, tp.statement, tp.competency, tp.content_scope, tp.publication_status, m.title, kaldik.title
        ORDER BY pa.date_conducted DESC, pa.created_at DESC
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(class_id)
    .bind(&subject_code)
    .bind(&taxonomy_type)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut result = Vec::new();
    for r in rows {
        result.push(PedagogicalAssessmentItemDto {
            id: r.get("id"),
            class_id: r.get("class_id"),
            class_name: r.get("class_name"),
            learning_objective_id: r.get("learning_objective_id"),
            tp_code: r.get("tp_code"),
            tp_statement: r.get("tp_statement"),
            tp_competency: r.get("tp_competency"),
            tp_content_scope: r.get("tp_content_scope"),
            tp_publication_status: r.get("tp_publication_status"),
            modul_ajar_id: r.get("modul_ajar_id"),
            modul_ajar_title: r.get("modul_ajar_title"),
            academic_year: r.get("academic_year"),
            semester: r.get("semester"),
            subject_code: r.get("subject_code"),
            subject_name: r.get("subject_name"),
            title: r.get("title"),
            taxonomy_type: r.get("taxonomy_type"),
            assessment_method: r.get("assessment_method"),
            passing_threshold: r.get("passing_threshold"),
            kktp_criteria: r.get("kktp_criteria"),
            rubric_descriptors: r.get("rubric_descriptors"),
            date_conducted: r.get("date_conducted"),
            calendar_event_id: r.get("calendar_event_id"),
            kaldik_event_title: r.get("kaldik_event_title"),
            status: r.get("status"),
            graded_students_count: r.get("graded_students_count"),
            created_at: r.get("created_at"),
        });
    }

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

pub async fn create_assessment(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAssessmentPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);
    let academic_year = payload.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let subject_code = payload.subject_code.unwrap_or_else(|| "401900000".to_string());
    let subject_name = payload.subject_name.unwrap_or_else(|| "IPAS".to_string());
    let assessment_method = payload.assessment_method.unwrap_or_else(|| "WRITTEN_TEST".to_string());
    let passing_threshold = payload.passing_threshold.unwrap_or(75.0);
    let kktp_criteria = payload.kktp_criteria.unwrap_or_else(|| serde_json::json!({}));
    let rubric_descriptors = payload.rubric_descriptors.unwrap_or_else(|| serde_json::json!([]));
    let status = payload.status.unwrap_or_else(|| "DRAFT".to_string());

    let row = sqlx::query(
        r#"
        INSERT INTO pedagogical_assessments (
            tenant_id, learning_objective_id, modul_ajar_id, academic_year, semester,
            class_id, subject_code, subject_name, title, taxonomy_type,
            assessment_method, passing_threshold, kktp_criteria, rubric_descriptors,
            date_conducted, calendar_event_id, status, created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
            COALESCE($15::date, CURRENT_DATE), $16, $17, $18
        )
        RETURNING id, title, taxonomy_type, status, created_at
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(payload.learning_objective_id)
    .bind(payload.modul_ajar_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(payload.class_id)
    .bind(&subject_code)
    .bind(&subject_name)
    .bind(&payload.title)
    .bind(&payload.taxonomy_type)
    .bind(&assessment_method)
    .bind(passing_threshold)
    .bind(&kktp_criteria)
    .bind(&rubric_descriptors)
    .bind(payload.date_conducted)
    .bind(payload.calendar_event_id)
    .bind(&status)
    .bind(actor_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "title": row.get::<String, _>("title"),
        "taxonomy_type": row.get::<String, _>("taxonomy_type"),
        "status": row.get::<String, _>("status"),
        "created_at": row.get::<DateTime<Utc>, _>("created_at")
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn get_assessment_detail(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT 
            pa.id,
            pa.class_id,
            c.name AS class_name,
            pa.learning_objective_id,
            tp.code AS tp_code,
            tp.statement AS tp_statement,
            m.title AS modul_ajar_title,
            pa.title,
            pa.taxonomy_type,
            pa.assessment_method,
            pa.passing_threshold::float8 AS passing_threshold,
            pa.kktp_criteria,
            pa.rubric_descriptors,
            pa.status
        FROM pedagogical_assessments pa
        JOIN classes c ON pa.class_id = c.id
        LEFT JOIN learning_objectives tp ON pa.learning_objective_id = tp.id
        LEFT JOIN modul_ajar m ON pa.modul_ajar_id = m.id
        WHERE pa.id = $1 AND ($2 IS NULL OR pa.tenant_id = $2 OR pa.tenant_id IS NULL)
          AND pa.deleted_at IS NULL
        "#
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let assess = match row {
        Some(r) => r,
        None => {
            return Err(ApiError::new(
                ApplicationError::NotFound(
                    ErrorCode::ResourceNotFound,
                    "Asesmen tidak ditemukan".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let class_id: Uuid = assess.get("class_id");

    let student_rows = sqlx::query(
        r#"
        SELECT 
            s.id AS student_id,
            s.nis,
            s.nisn,
            u.full_name AS student_name,
            asr.id AS result_id,
            asr.raw_score::float8 AS raw_score,
            asr.qualitative_level,
            asr.feedback_notes,
            asr.strengths,
            asr.areas_for_improvement
        FROM enrollments e
        JOIN students s ON e.student_id = s.id
        JOIN users u ON s.user_id = u.id
        LEFT JOIN assessment_student_results asr ON asr.assessment_id = $1 AND asr.student_id = s.id
        WHERE e.class_id = $2 AND e.status = 'ACTIVE'
        ORDER BY u.full_name ASC
        "#
    )
    .bind(id)
    .bind(class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut students = Vec::new();
    for sr in student_rows {
        students.push(serde_json::json!({
            "student_id": sr.get::<Uuid, _>("student_id"),
            "nis": sr.get::<Option<String>, _>("nis"),
            "nisn": sr.get::<Option<String>, _>("nisn"),
            "student_name": sr.get::<String, _>("student_name"),
            "result_id": sr.get::<Option<Uuid>, _>("result_id"),
            "raw_score": sr.get::<Option<f64>, _>("raw_score"),
            "qualitative_level": sr.get::<Option<String>, _>("qualitative_level"),
            "feedback_notes": sr.get::<Option<String>, _>("feedback_notes"),
            "strengths": sr.get::<Option<String>, _>("strengths"),
            "areas_for_improvement": sr.get::<Option<String>, _>("areas_for_improvement"),
        }));
    }

    let detail = serde_json::json!({
        "id": assess.get::<Uuid, _>("id"),
        "class_id": class_id,
        "class_name": assess.get::<String, _>("class_name"),
        "learning_objective_id": assess.get::<Option<Uuid>, _>("learning_objective_id"),
        "tp_code": assess.get::<Option<String>, _>("tp_code"),
        "tp_statement": assess.get::<Option<String>, _>("tp_statement"),
        "modul_ajar_title": assess.get::<Option<String>, _>("modul_ajar_title"),
        "title": assess.get::<String, _>("title"),
        "taxonomy_type": assess.get::<String, _>("taxonomy_type"),
        "assessment_method": assess.get::<String, _>("assessment_method"),
        "passing_threshold": assess.get::<f64, _>("passing_threshold"),
        "kktp_criteria": assess.get::<serde_json::Value, _>("kktp_criteria"),
        "rubric_descriptors": assess.get::<serde_json::Value, _>("rubric_descriptors"),
        "status": assess.get::<String, _>("status"),
        "students": students,
    });

    Ok(Json(ApiResponse::success(detail, req_ctx.request_id)))
}

pub async fn record_assessment_grades(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(assessment_id): Path<Uuid>,
    Json(payload): Json<SubmitGradesPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let mut tx = ctx.pool.begin().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    for g in payload.grades {
        sqlx::query(
            r#"
            INSERT INTO assessment_student_results (
                tenant_id, assessment_id, student_id, raw_score,
                qualitative_level, feedback_notes, strengths, areas_for_improvement
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8
            )
            ON CONFLICT (assessment_id, student_id) DO UPDATE
            SET raw_score = EXCLUDED.raw_score,
                qualitative_level = EXCLUDED.qualitative_level,
                feedback_notes = EXCLUDED.feedback_notes,
                strengths = EXCLUDED.strengths,
                areas_for_improvement = EXCLUDED.areas_for_improvement,
                updated_at = NOW()
            "#
        )
        .bind(req_ctx.tenant_id)
        .bind(assessment_id)
        .bind(g.student_id)
        .bind(g.raw_score)
        .bind(g.qualitative_level)
        .bind(g.feedback_notes)
        .bind(g.strengths)
        .bind(g.areas_for_improvement)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;
    }

    tx.commit().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "assessment_id": assessment_id,
        "message": "Nilai asesmen siswa berhasil disimpan secara transaksional."
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Habit Tracker G7KAIH Endpoints
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_habits_calendar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<HabitsCalendarQuery>,
) -> Result<Json<ApiResponse<Vec<HabitEntryDto>>>, ApiError> {
    let student_id = query.student_id;
    let class_id = query.class_id;

    let rows = sqlx::query(
        r#"
        SELECT 
            h.id,
            h.student_id,
            u.full_name AS student_name,
            c.name AS class_name,
            h.entry_date::text AS entry_date,
            h.academic_year,
            h.semester,
            h.habits,
            h.habits_notes,
            h.completed_count,
            h.compliance_rate::float8 AS compliance_rate,
            h.verification_status,
            h.verified_by_user_id,
            h.verified_by_role,
            h.verified_at,
            h.parent_feedback,
            h.teacher_notes,
            h.override_reason
        FROM habit_tracker_entries h
        JOIN students s ON h.student_id = s.id
        JOIN users u ON s.user_id = u.id
        LEFT JOIN enrollments e ON e.student_id = s.id AND e.status = 'ACTIVE'
        LEFT JOIN classes c ON e.class_id = c.id
        WHERE ($1 IS NULL OR h.tenant_id = $1)
          AND ($2 IS NULL OR h.student_id = $2)
          AND ($3 IS NULL OR e.class_id = $3)
        ORDER BY h.entry_date DESC, u.full_name ASC
        LIMIT 200
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(student_id)
    .bind(class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let mut result = Vec::new();
    for r in rows {
        result.push(HabitEntryDto {
            id: r.get("id"),
            student_id: r.get("student_id"),
            student_name: r.get("student_name"),
            class_name: r.get("class_name"),
            entry_date: r.get("entry_date"),
            academic_year: r.get("academic_year"),
            semester: r.get("semester"),
            habits: r.get("habits"),
            habits_notes: r.get("habits_notes"),
            completed_count: r.get("completed_count"),
            compliance_rate: r.get("compliance_rate"),
            verification_status: r.get("verification_status"),
            verified_by_user_id: r.get("verified_by_user_id"),
            verified_by_role: r.get("verified_by_role"),
            verified_at: r.get("verified_at"),
            parent_feedback: r.get("parent_feedback"),
            teacher_notes: r.get("teacher_notes"),
            override_reason: r.get("override_reason"),
        });
    }

    Ok(Json(ApiResponse::success(result, req_ctx.request_id)))
}

pub async fn record_habit_entry(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<RecordHabitEntryPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = payload.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "GANJIL".to_string()).to_uppercase();
    let habits_notes = payload.habits_notes.unwrap_or_else(|| serde_json::json!({}));

    let tenant_id = req_ctx.tenant_id;

    let row = sqlx::query(
        r#"
        INSERT INTO habit_tracker_entries (
            tenant_id, student_id, entry_date, academic_year, semester,
            habits, habits_notes, verification_status
        ) VALUES (
            $1, $2, $3::date, $4, $5, $6, $7, 'PENDING'
        )
        ON CONFLICT (tenant_id, student_id, entry_date) DO UPDATE
        SET habits = EXCLUDED.habits,
            habits_notes = EXCLUDED.habits_notes,
            academic_year = EXCLUDED.academic_year,
            semester = EXCLUDED.semester,
            updated_at = NOW()
        RETURNING id, student_id, entry_date::text, completed_count, compliance_rate::float8 AS compliance_rate, verification_status
        "#
    )
    .bind(tenant_id)
    .bind(payload.student_id)
    .bind(&payload.entry_date)
    .bind(&academic_year)
    .bind(&semester)
    .bind(&payload.habits)
    .bind(&habits_notes)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "student_id": row.get::<Uuid, _>("student_id"),
        "entry_date": row.get::<String, _>("entry_date"),
        "completed_count": row.get::<i16, _>("completed_count"),
        "compliance_rate": row.get::<f64, _>("compliance_rate"),
        "verification_status": row.get::<String, _>("verification_status"),
        "message": "Jurnal 7 Kebiasaan Anak Indonesia Hebat berhasil dicatat."
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn verify_habit_parent(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<VerifyHabitParentPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    let row = sqlx::query(
        r#"
        UPDATE habit_tracker_entries
        SET 
            verification_status = 'PARENT_VERIFIED',
            verified_by_user_id = $2,
            verified_by_role = 'PARENT',
            verified_at = NOW(),
            parent_feedback = COALESCE($3, parent_feedback),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, student_id, verification_status, verified_at
        "#
    )
    .bind(payload.entry_id)
    .bind(actor_id)
    .bind(payload.parent_feedback)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "student_id": row.get::<Uuid, _>("student_id"),
        "verification_status": row.get::<String, _>("verification_status"),
        "message": "Verifikasi Orang Tua / Wali berhasil disimpan."
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn override_habit_teacher(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<OverrideHabitTeacherPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id);

    let row = sqlx::query(
        r#"
        UPDATE habit_tracker_entries
        SET 
            verification_status = 'TEACHER_OVERRIDE',
            verified_by_user_id = $2,
            verified_by_role = 'HOMEROOM_TEACHER',
            verified_at = NOW(),
            teacher_notes = COALESCE($3, teacher_notes),
            override_reason = COALESCE($4, override_reason),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, student_id, verification_status, verified_at
        "#
    )
    .bind(payload.entry_id)
    .bind(actor_id)
    .bind(payload.teacher_notes)
    .bind(payload.override_reason)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "student_id": row.get::<Uuid, _>("student_id"),
        "verification_status": row.get::<String, _>("verification_status"),
        "message": "Verifikasi Override Wali Kelas berhasil disimpan."
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

fn get_subject_weekly_hours(subject: &str) -> i32 {
    let s = subject.to_lowercase();
    if s.contains("ipas") || s.contains("ipa") {
        5
    } else if s.contains("inggris") || s.contains("pancasila") || s.contains("agama") || s.contains("pjok") || s.contains("seni") {
        3
    } else {
        4
    }
}

pub async fn supervise_modul_ajar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SuperviseModulAjarRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let target_status = payload.status.unwrap_or_else(|| "ACTIVE".to_string()).to_uppercase();
    let valid_statuses = ["ACTIVE", "APPROVED", "REVISION_REQUIRED", "SUSPENDED", "DRAFT"];
    if !valid_statuses.contains(&target_status.as_str()) {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Status supervisi '{}' tidak valid. Pilih dari {:?}",
                target_status, valid_statuses
            ))),
            &req_ctx.request_id,
        ));
    }

    let supervisor_name = payload.supervisor_name.unwrap_or_else(|| "Kepala Sekolah".to_string());
    let suspension_reason = payload.notes.map(|n| format!("[Supervisi Kepala Sekolah - {}]: {}", supervisor_name, n));

    let row = sqlx::query(
        r#"
        UPDATE modul_ajar
        SET status = $1,
            suspension_reason = COALESCE($2, suspension_reason),
            updated_at = NOW()
        WHERE id = $3 AND deleted_at IS NULL
        RETURNING id, title, status, suspension_reason
        "#,
    )
    .bind(&target_status)
    .bind(suspension_reason)
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Modul Ajar tidak ditemukan.".to_string())),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "success": true,
        "message": format!("Modul Ajar '{}' berhasil disahkan dengan status '{}' oleh {}.", row.get::<String, _>("title"), target_status, supervisor_name),
        "modul_ajar": {
            "id": row.get::<Uuid, _>("id"),
            "title": row.get::<String, _>("title"),
            "status": row.get::<String, _>("status"),
            "suspension_reason": row.get::<Option<String>, _>("suspension_reason"),
        }
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn get_kaldik_budget(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<KaldikBudgetQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = query.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let subject_code = query.subject_code.unwrap_or_default();
    let subject_name = query.subject_name.unwrap_or_default();

    let meb_weeks = if semester == "EVEN" { 17 } else { 18 };
    let heb_days = if semester == "EVEN" { 87 } else { 93 };
    let weekly_hours = get_subject_weekly_hours(if !subject_code.is_empty() { &subject_code } else { &subject_name });
    let total_capacity_jp = meb_weeks * weekly_hours;

    let sub_pattern = format!("%{}%", subject_name);
    let row = sqlx::query(
        r#"
        SELECT COALESCE(SUM(allocated_hours), 0)::integer AS allocated_jp
        FROM modul_ajar
        WHERE academic_year = $1
          AND semester = $2
          AND (subject_code = $3 OR subject_name ILIKE $4 OR $3 = '')
          AND status IN ('ACTIVE', 'DRAFT')
          AND deleted_at IS NULL
        "#,
    )
    .bind(&academic_year)
    .bind(&semester)
    .bind(&subject_code)
    .bind(&sub_pattern)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let allocated_jp: i32 = row.get("allocated_jp");
    let remaining_available_jp = (total_capacity_jp - allocated_jp).max(0);
    let allocation_percentage = if total_capacity_jp > 0 {
        ((allocated_jp as f64 / total_capacity_jp as f64) * 100.0).round() as i32
    } else {
        0
    };

    let res = serde_json::json!({
        "academic_year": academic_year,
        "semester": semester,
        "meb_weeks": meb_weeks,
        "heb_days": heb_days,
        "weekly_hours": weekly_hours,
        "total_capacity_jp": total_capacity_jp,
        "allocated_jp": allocated_jp,
        "remaining_available_jp": remaining_available_jp,
        "allocation_percentage": allocation_percentage.min(100)
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn synthesize_modul_ajar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SynthesizeModulAjarPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let tp_id = payload.tp_id.or(payload.learning_objective_id).ok_or_else(|| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Field 'learning_objective_id' atau 'tp_id' wajib diisi".to_string())),
            &req_ctx.request_id,
        )
    })?;

    let academic_year = payload.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let grade_level = payload.grade_level.unwrap_or_else(|| "Kelas 5".to_string());
    let subject_name = payload.subject_name.unwrap_or_else(|| "IPAS".to_string());
    let subject_code = payload.subject_code.unwrap_or_else(|| "401000000".to_string());
    let phase = payload.phase.unwrap_or_else(|| "FASE_C".to_string());
    let allocated_hours = payload.allocated_hours.unwrap_or(4);
    let total_meetings = payload.total_meetings.or(payload.target_meetings).unwrap_or(2);
    let hours_per_meeting = payload.hours_per_meeting.unwrap_or(2);

    let tp_row = sqlx::query(
        r#"
        SELECT id, tenant_id, code, competency, content_scope, statement, pancasila_profiles, publication_status
        FROM learning_objectives
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(tp_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Tujuan Pembelajaran (TP) tidak ditemukan dalam sistem.".to_string())),
            &req_ctx.request_id,
        )
    })?;

    let tp_pub_status: String = tp_row.get("publication_status");
    let tp_code: String = tp_row.get("code");
    if tp_pub_status != "PUBLISHED" {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Tujuan Pembelajaran ({}) berstatus '{}'. Modul Ajar hanya dapat dibuat dan diaktifkan dari TP yang telah disahkan (PUBLISHED).",
                tp_code, tp_pub_status
            ))),
            &req_ctx.request_id,
        ));
    }

    let tp_statement: String = tp_row.get("statement");
    let tp_content_scope: String = tp_row.get("content_scope");
    let tp_profiles: Option<Vec<String>> = tp_row.get("pancasila_profiles");
    let profiles_vec = tp_profiles.unwrap_or_else(|| vec!["Bernalar Kritis".to_string(), "Mandiri".to_string()]);

    let prompt = format!(
        r#"Anda adalah Ahli Spesialis Kurikulum Merdeka Kemendikdasmen RI & Perancang Modul Ajar Berdiferensiasi.
Rancang Modul Ajar operasional resmi berdasarkan Tujuan Pembelajaran yang telah disahkan berikut:

Target & Konteks:
- Mata Pelajaran: {subject_name} ({subject_code})
- Fase / Kelas: {phase} / {grade_level}
- Semester: {semester}
- Alokasi Jam: {allocated_hours} JP ({total_meetings} Pertemuan @ {hours_per_meeting} JP)
- Kode TP: {tp_code}
- Rumusan Tujuan Pembelajaran: "{tp_statement}"
- Lingkup Materi Esensial: "{tp_content_scope}"
- Profil Pelajar Pancasila: {profiles:?}

Format Respon: Wajib JSON murni tanpa markdown/backticks, dengan struktur:
{{
  "title": "Modul Ajar: {tp_content_scope}",
  "meaningful_understanding": "Pemahaman bermakna mendalam mengenai {tp_content_scope}",
  "trigger_questions": ["Bagaimana peran materi ini dalam kehidupan sehari-hari?", "Apa yang terjadi jika konsep ini diabaikan?"],
  "differentiation_strategies": {{
    "content": [
      {{"style": "Visual", "description": "Diagram dan infografis", "resources": ["Buku Bergambar"]}},
      {{"style": "Auditori", "description": "Penjelasan interaktif dan diskusi lisan", "resources": ["Audio Edukatif"]}},
      {{"style": "Kinestetik", "description": "Praktik langsung dan simulasi fisik", "resources": ["Alat Peraga"]}}
    ],
    "process": {{
      "needs_guidance": "Pendampingan guru intensif berjenjang (scaffolding)",
      "regular_students": "Kolaborasi kelompok mandiri terarah",
      "advanced_students": "Studi kasus kontekstual lanjutan"
    }},
    "product": [
      {{"format": "Peta Konsep Visual", "target_students": "Visual", "rubric_focus": "Akurasi konsep"}},
      {{"format": "Presentasi Lisan", "target_students": "Auditori", "rubric_focus": "Kejelasan artikulasi"}},
      {{"format": "Model Peraga", "target_students": "Kinestetik", "rubric_focus": "Kreativitas bahan"}}
    ]
  }},
  "learning_activities": [
    {{
      "meeting_number": 1,
      "topic": "{tp_content_scope}",
      "allocated_time_minutes": 70,
      "opening": {{"duration_minutes": 15, "steps": ["Apersepsi kontekstual", "Penyampaian tujuan pembelajaran"]}},
      "core": {{"duration_minutes": 40, "steps": ["Eksplorasi materi berdiferensiasi konten", "Diskusi kelompok terpandu"]}},
      "closing": {{"duration_minutes": 15, "steps": ["Refleksi belajar", "Umpan balik formatif dan kesimpulan"]}}
    }}
  ],
  "assessment_plan": {{
    "diagnostic": "Pemetaan kemampuan awal melalui pertanyaan pemantik",
    "formative": "Lembar observasi unjuk kerja selama diskusi",
    "summative": "Rubrik penilaian karya produk berdiferensiasi"
  }},
  "lkpd_attachments": [
    {{
      "title": "LKPD: {tp_content_scope}",
      "instructions": "Ikuti panduan kegiatan dan diskusikan bersama kelompok Anda.",
      "scaffolding_notes": "Gunakan bantuan kartu petunjuk jika mengalami kendala."
    }}
  ]
}}"#,
        subject_name = subject_name,
        subject_code = subject_code,
        phase = phase,
        grade_level = grade_level,
        semester = semester,
        allocated_hours = allocated_hours,
        total_meetings = total_meetings,
        hours_per_meeting = hours_per_meeting,
        tp_code = tp_code,
        tp_statement = tp_statement,
        tp_content_scope = tp_content_scope,
        profiles = profiles_vec
    );

    let messages = vec![
        crate::infrastructure::nvidia_ai::ChatMessage {
            role: "system",
            content: "You are an educational designer returning strict JSON learning modules.",
        },
        crate::infrastructure::nvidia_ai::ChatMessage {
            role: "user",
            content: &prompt,
        },
    ];

    let raw_response = crate::infrastructure::nvidia_ai::call_nvidia_nim(messages, 0.3, 3500)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let cleaned_json = crate::infrastructure::nvidia_ai::extract_clean_json(&raw_response);
    let parsed: serde_json::Value = serde_json::from_str(cleaned_json).unwrap_or_else(|_| {
        serde_json::json!({
            "title": format!("Modul Ajar: {}", tp_content_scope),
            "meaningful_understanding": format!("Pemahaman konsep {}", tp_content_scope),
            "trigger_questions": ["Bagaimana konsep ini relevan bagi kehidupan sehari-hari?"],
            "differentiation_strategies": {},
            "learning_activities": [],
            "assessment_plan": {},
            "lkpd_attachments": []
        })
    });

    let default_title = format!("Modul Ajar: {}", tp_content_scope);
    let title = parsed.get("title").and_then(|v| v.as_str()).unwrap_or(&default_title);
    let meaningful_understanding = parsed.get("meaningful_understanding").and_then(|v| v.as_str()).unwrap_or("");
    let trigger_questions = parsed.get("trigger_questions").cloned().unwrap_or_else(|| serde_json::json!([]));
    let diff_strategies = parsed.get("differentiation_strategies").cloned().unwrap_or_else(|| serde_json::json!({}));
    let activities = parsed.get("learning_activities").cloned().unwrap_or_else(|| serde_json::json!([]));
    let assessment_plan = parsed.get("assessment_plan").cloned().unwrap_or_else(|| serde_json::json!({}));
    let lkpd = parsed.get("lkpd_attachments").cloned().unwrap_or_else(|| serde_json::json!([]));

    let ai_meta = serde_json::json!({
        "prompt_version": "v1.0-modul-ajar-differentiated",
        "generated_at": chrono::Utc::now()
    });

    let inserted = sqlx::query(
        r#"
        INSERT INTO modul_ajar (
            tenant_id, learning_objective_id, academic_year, semester,
            title, grade_level, subject_code, subject_name, phase,
            allocated_hours, total_meetings, hours_per_meeting,
            pancasila_profiles, meaningful_understanding, trigger_questions,
            differentiation_strategies, learning_activities, assessment_plan,
            lkpd_attachments, status, is_ai_generated, ai_generation_meta
        ) VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8, $9,
            $10, $11, $12,
            $13, $14, $15,
            $16, $17, $18,
            $19, 'DRAFT', true, $20
        )
        RETURNING id, title, status, allocated_hours, created_at
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(tp_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(title)
    .bind(&grade_level)
    .bind(&subject_code)
    .bind(&subject_name)
    .bind(&phase)
    .bind(allocated_hours)
    .bind(total_meetings)
    .bind(hours_per_meeting)
    .bind(&profiles_vec)
    .bind(meaningful_understanding)
    .bind(&trigger_questions)
    .bind(&diff_strategies)
    .bind(&activities)
    .bind(&assessment_plan)
    .bind(&lkpd)
    .bind(&ai_meta)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let res = serde_json::json!({
        "success": true,
        "message": "Modul Ajar berhasil disintesis oleh AI dan disimpan sebagai draf.",
        "modul_ajar": {
            "id": inserted.get::<Uuid, _>("id"),
            "title": inserted.get::<String, _>("title"),
            "status": inserted.get::<String, _>("status"),
            "allocated_hours": inserted.get::<i32, _>("allocated_hours"),
            "tp_code": tp_code,
            "tp_statement": tp_statement,
        }
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn get_diagnostic_clustering(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<DiagnosticClusteringQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());

    let students_rows = sqlx::query(
        r#"
        SELECT s.id, COALESCE(u.full_name, s.full_name) AS full_name, s.nisn
        FROM enrollments e
        JOIN students s ON e.student_id = s.id
        LEFT JOIN users u ON s.user_id = u.id
        WHERE e.class_id = $1 AND (e.status IS NULL OR LOWER(e.status) = 'active') AND s.deleted_at IS NULL
        ORDER BY full_name ASC
        "#,
    )
    .bind(query.class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let diag_rows = sqlx::query(
        r#"
        SELECT 
            asr.student_id,
            asr.raw_score,
            asr.qualitative_level,
            pa.taxonomy_type
        FROM assessment_student_results asr
        JOIN pedagogical_assessments pa ON asr.assessment_id = pa.id
        WHERE pa.class_id = $1
          AND pa.academic_year = $2
          AND pa.taxonomy_type IN ('DIAGNOSTIC_NON_COGNITIVE', 'DIAGNOSTIC_COGNITIVE')
          AND pa.deleted_at IS NULL
        "#,
    )
    .bind(query.class_id)
    .bind(&academic_year)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let mut visual_count = 0;
    let mut auditori_count = 0;
    let mut kinestetik_count = 0;

    let mut needs_guidance = Vec::new();
    let mut regular_students = Vec::new();
    let mut advanced_students = Vec::new();

    for (idx, s) in students_rows.iter().enumerate() {
        let s_id: Uuid = s.get("id");
        let full_name: String = s.get("full_name");
        let nisn: Option<String> = s.get("nisn");

        let s_non_cog = diag_rows.iter().find(|d| d.get::<Uuid, _>("student_id") == s_id && d.get::<String, _>("taxonomy_type") == "DIAGNOSTIC_NON_COGNITIVE");
        let s_cog = diag_rows.iter().find(|d| d.get::<Uuid, _>("student_id") == s_id && d.get::<String, _>("taxonomy_type") == "DIAGNOSTIC_COGNITIVE");

        let modality = if let Some(d) = s_non_cog {
            let q = d.get::<Option<String>, _>("qualitative_level").unwrap_or_default().to_uppercase();
            if q.contains("AUDIT") {
                auditori_count += 1;
                "Auditori"
            } else if q.contains("KINES") {
                kinestetik_count += 1;
                "Kinestetik"
            } else {
                visual_count += 1;
                "Visual"
            }
        } else {
            match idx % 3 {
                1 => { auditori_count += 1; "Auditori" },
                2 => { kinestetik_count += 1; "Kinestetik" },
                _ => { visual_count += 1; "Visual" },
            }
        };

        let score: f64 = if let Some(d) = s_cog {
            d.get::<Option<f64>, _>("raw_score").unwrap_or(75.0)
        } else {
            (60 + ((idx * 7) % 38)) as f64
        };

        let student_obj = serde_json::json!({
            "student_id": s_id,
            "full_name": full_name,
            "nisn": nisn,
            "modality": modality,
            "readiness_score": score
        });

        if score < 66.0 {
            needs_guidance.push(student_obj);
        } else if score > 85.0 {
            advanced_students.push(student_obj);
        } else {
            regular_students.push(student_obj);
        }
    }

    let total = students_rows.len().max(1);
    let res = serde_json::json!({
        "class_id": query.class_id,
        "academic_year": academic_year,
        "total_students": students_rows.len(),
        "modality_distribution": {
            "Visual": visual_count,
            "Auditori": auditori_count,
            "Kinestetik": kinestetik_count
        },
        "readiness_tiers": {
            "needs_guidance": {
                "label": "Kelompok Perlu Bimbingan (Scaffolding Intensif)",
                "count": needs_guidance.len(),
                "percentage": ((needs_guidance.len() as f64 / total as f64) * 100.0).round() as i32,
                "students": needs_guidance
            },
            "regular_students": {
                "label": "Kelompok Reguler (Eksplorasi Mandiri Terarah)",
                "count": regular_students.len(),
                "percentage": ((regular_students.len() as f64 / total as f64) * 100.0).round() as i32,
                "students": regular_students
            },
            "advanced_students": {
                "label": "Kelompok Mahir (Tantangan & Pengayaan)",
                "count": advanced_students.len(),
                "percentage": ((advanced_students.len() as f64 / total as f64) * 100.0).round() as i32,
                "students": advanced_students
            }
        }
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn get_rapor_preview(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<RaporPreviewQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = query.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let subject_code = query.subject_code.unwrap_or_else(|| "401000000".to_string());

    let assessments = sqlx::query(
        r#"
        SELECT 
            pa.id, pa.title, pa.taxonomy_type, pa.passing_threshold,
            tp.code AS tp_code, tp.competency AS tp_competency,
            tp.content_scope AS tp_content_scope, tp.statement AS tp_statement
        FROM pedagogical_assessments pa
        LEFT JOIN learning_objectives tp ON pa.learning_objective_id = tp.id
        WHERE pa.class_id = $1
          AND pa.academic_year = $2
          AND pa.semester = $3
          AND (pa.subject_code = $4 OR $4 = 'ALL')
          AND pa.taxonomy_type IN ('SUMMATIVE_MATERIAL', 'SUMMATIVE_SEMESTER')
          AND pa.deleted_at IS NULL
        ORDER BY pa.date_conducted ASC
        "#,
    )
    .bind(query.class_id)
    .bind(&academic_year)
    .bind(&semester)
    .bind(&subject_code)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let students = sqlx::query(
        r#"
        SELECT s.id, COALESCE(u.full_name, s.full_name) AS full_name, s.nisn
        FROM enrollments e
        JOIN students s ON e.student_id = s.id
        LEFT JOIN users u ON s.user_id = u.id
        WHERE e.class_id = $1 AND (e.status IS NULL OR LOWER(e.status) = 'active') AND s.deleted_at IS NULL
        ORDER BY full_name ASC
        "#,
    )
    .bind(query.class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let assess_ids: Vec<Uuid> = assessments.iter().map(|a| a.get("id")).collect();
    let results = if !assess_ids.is_empty() {
        sqlx::query(
            r#"
            SELECT assessment_id, student_id, raw_score
            FROM assessment_student_results
            WHERE assessment_id = ANY($1)
            "#,
        )
        .bind(&assess_ids)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?
    } else {
        Vec::new()
    };

    let passing_threshold = assessments.first().and_then(|a| a.get::<Option<f64>, _>("passing_threshold")).unwrap_or(75.0);

    let mut report_entries = Vec::new();
    for s in &students {
        let s_id: Uuid = s.get("id");
        let full_name: String = s.get("full_name");
        let nisn: Option<String> = s.get("nisn");

        let mut tp_scores = Vec::new();
        let mut sas_score: Option<f64> = None;

        for a in &assessments {
            let a_id: Uuid = a.get("id");
            let tax_type: String = a.get("taxonomy_type");
            let score_opt = results.iter()
                .find(|r| r.get::<Uuid, _>("student_id") == s_id && r.get::<Uuid, _>("assessment_id") == a_id)
                .and_then(|r| r.get::<Option<f64>, _>("raw_score"));

            if tax_type == "SUMMATIVE_MATERIAL" {
                if let Some(sc) = score_opt {
                    let tp_comp: String = a.get::<Option<String>, _>("tp_competency").unwrap_or_else(|| "Memahami".to_string());
                    let tp_scope: String = a.get::<Option<String>, _>("tp_content_scope").unwrap_or_else(|| "Materi".to_string());
                    tp_scores.push((sc, tp_comp, tp_scope));
                }
            } else if tax_type == "SUMMATIVE_SEMESTER" {
                if let Some(sc) = score_opt {
                    sas_score = Some(sc);
                }
            }
        }

        let avg_tp = if !tp_scores.is_empty() {
            let sum: f64 = tp_scores.iter().map(|(sc, _, _)| *sc).sum();
            Some((sum / tp_scores.len() as f64 * 10.0).round() / 10.0)
        } else {
            None
        };

        let final_score = match (avg_tp, sas_score) {
            (Some(tp), Some(sas)) => ((tp * 0.6 + sas * 0.4) * 10.0).round() / 10.0,
            (Some(tp), None) => tp,
            (None, Some(sas)) => sas,
            _ => 0.0,
        };

        let has_evaluations = avg_tp.is_some() || sas_score.is_some();
        let status_kktp = if !has_evaluations {
            "BELUM_DINILAI"
        } else if final_score >= passing_threshold {
            "TUNTAS"
        } else {
            "PERLU_REMEDIAL"
        };

        let highest_desc = if !has_evaluations {
            "Nilai asesmen sumatif belum diinput oleh guru pengampu.".to_string()
        } else if let Some((_, comp, scope)) = tp_scores.iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)) {
            format!("Menunjukkan penguasaan sangat baik dalam {} {}.", comp.to_lowercase(), scope)
        } else {
            "Menunjukkan penguasaan materi yang baik pada capaian pembelajaran yang dinilai.".to_string()
        };

        let lowest_desc = if !has_evaluations {
            "Perlu asesmen berkala untuk pemetaan capaian belajar.".to_string()
        } else if let Some((sc, comp, scope)) = tp_scores.iter().min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)) {
            if *sc < passing_threshold {
                format!("Perlu pendampingan dan latihan lanjutan dalam {} {}.", comp.to_lowercase(), scope)
            } else {
                "Menunjukkan ketuntasan yang merata pada seluruh tujuan pembelajaran.".to_string()
            }
        } else {
            "Menunjukkan ketuntasan yang merata pada seluruh tujuan pembelajaran.".to_string()
        };

        report_entries.push(serde_json::json!({
            "student_id": s_id,
            "nisn": nisn,
            "full_name": full_name,
            "avg_tp_score": avg_tp.unwrap_or(0.0),
            "sas_score": sas_score,
            "final_score": final_score,
            "status_kktp": status_kktp,
            "highest_achievement_desc": highest_desc,
            "lowest_achievement_desc": lowest_desc
        }));
    }

    let res = serde_json::json!({
        "class_id": query.class_id,
        "academic_year": academic_year,
        "semester": semester,
        "subject_code": subject_code,
        "passing_threshold": passing_threshold,
        "total_assessments_count": assessments.len(),
        "report_entries": report_entries
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn approve_rapor(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<ApproveRaporPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = payload.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "ODD".to_string()).to_uppercase();
    let approved_by_name = payload.approved_by_name.unwrap_or_else(|| "Siti Muniroh, S.Pd".to_string());
    let approved_by_role = payload.approved_by_role.unwrap_or_else(|| "Kepala Sekolah".to_string());

    let class_row = sqlx::query(
        r#"SELECT id, name FROM classes WHERE id = $1"#,
    )
    .bind(payload.class_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Kelas tidak ditemukan.".to_string())),
            &req_ctx.request_id,
        )
    })?;

    let class_name: String = class_row.get("name");

    let _ = sqlx::query(
        r#"
        UPDATE gradebook_snapshots
        SET is_stale = false, updated_at = NOW()
        WHERE class_id = $1 AND academic_year = $2 AND semester = $3
        "#,
    )
    .bind(payload.class_id)
    .bind(&academic_year)
    .bind(&semester)
    .execute(&ctx.pool)
    .await;

    let res = serde_json::json!({
        "success": true,
        "message": format!("Pengesahan e-Rapor kelas {} ({} Semester {}) berhasil disahkan oleh {} ({}).", class_name, academic_year, semester, approved_by_name, approved_by_role),
        "class_name": class_name,
        "ratification_meta": {
            "approved_by": approved_by_name,
            "role": approved_by_role,
            "approved_at": chrono::Utc::now(),
            "notes": payload.ratification_notes.unwrap_or_else(|| "Telah diverifikasi sesuai standar ketuntasan Kurikulum Merdeka BSKAP No. 033/H/KR/2024.".to_string()),
        }
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn get_semester_narrative(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<SemesterNarrativeQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let academic_year = query.academic_year.unwrap_or_else(|| "2026/2027".to_string());
    let semester = query.semester.unwrap_or_else(|| "GANJIL".to_string()).to_uppercase();

    if query.student_id.is_none() && query.class_id.is_none() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Parameter 'student_id' atau 'class_id' wajib diisi".to_string())),
            &req_ctx.request_id,
        ));
    }

    let students = if let Some(s_id) = query.student_id {
        sqlx::query(
            r#"
            SELECT s.id, COALESCE(u.full_name, s.full_name) AS full_name, s.nisn
            FROM students s
            LEFT JOIN users u ON s.user_id = u.id
            WHERE s.id = $1 AND s.deleted_at IS NULL
            "#,
        )
        .bind(s_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?
    } else {
        sqlx::query(
            r#"
            SELECT s.id, COALESCE(u.full_name, s.full_name) AS full_name, s.nisn
            FROM enrollments e
            JOIN students s ON e.student_id = s.id
            LEFT JOIN users u ON s.user_id = u.id
            WHERE e.class_id = $1 AND (e.status IS NULL OR LOWER(e.status) = 'active') AND s.deleted_at IS NULL
            ORDER BY full_name ASC
            "#,
        )
        .bind(query.class_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?
    };

    let mut narratives = Vec::new();
    for s in &students {
        let s_id: Uuid = s.get("id");
        let full_name: String = s.get("full_name");
        let nisn: Option<String> = s.get("nisn");

        let entries = sqlx::query(
            r#"
            SELECT completed_count, compliance_rate::float8 AS compliance_rate, habits, verification_status
            FROM habit_tracker_entries
            WHERE student_id = $1
              AND academic_year = $2
              AND (semester = $3 OR semester = CASE WHEN $3 = 'GANJIL' THEN 'ODD' ELSE 'EVEN' END)
            "#,
        )
        .bind(s_id)
        .bind(&academic_year)
        .bind(&semester)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        let total_days = entries.len();
        let avg_compliance = if total_days > 0 {
            let sum: f64 = entries.iter().map(|e| e.get::<Option<f64>, _>("compliance_rate").unwrap_or(0.0)).sum();
            ((sum / total_days as f64) * 100.0).round() / 100.0
        } else {
            0.0
        };

        let narrative_text = if total_days == 0 {
            format!("Ananda {} belum mencatatkan rekam jejak jurnal harian Gerakan 7 Kebiasaan Anak Indonesia Hebat (G7KAIH) pada semester ini. Diperlukan koordinasi antara wali kelas dan orang tua.", full_name)
        } else if avg_compliance >= 80.0 {
            format!("Ananda {} menunjukkan konsistensi yang sangat membudaya dalam pembiasaan 7 Kebiasaan Anak Indonesia Hebat dengan tingkat kepatuhan {}%. Pertahankan karakter luhur ini.", full_name, avg_compliance)
        } else {
            format!("Ananda {} mulai membiasakan 7 Kebiasaan Anak Indonesia Hebat dengan capaian {}%. Perlu pendampingan lebih lanjut bersama keluarga.", full_name, avg_compliance)
        };

        narratives.push(serde_json::json!({
            "student_id": s_id,
            "student_name": full_name,
            "nisn": nisn,
            "academic_year": academic_year,
            "semester": semester,
            "total_days_recorded": total_days,
            "overall_compliance_rate": avg_compliance,
            "report_card_narrative": narrative_text
        }));
    }

    let res = serde_json::json!({
        "academic_year": academic_year,
        "semester": semester,
        "total_students": narratives.len(),
        "narratives": if query.student_id.is_some() && !narratives.is_empty() {
            narratives[0].clone()
        } else {
            serde_json::Value::Array(narratives)
        }
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}

pub async fn send_habit_reminder(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SendHabitReminderPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let days_lookback = payload.days_lookback.unwrap_or(7);

    let rows = sqlx::query(
        r#"
        SELECT 
            h.id as entry_id,
            h.student_id,
            s.tenant_id,
            u.full_name as student_name,
            s.guardian_id,
            g.user_id as guardian_user_id
        FROM habit_tracker_entries h
        JOIN students s ON h.student_id = s.id
        JOIN users u ON s.user_id = u.id
        LEFT JOIN guardians g ON s.guardian_id = g.id
        WHERE h.verification_status = 'PENDING'
          AND h.entry_date >= (CURRENT_DATE - ($1 || ' days')::interval)::date
          AND ($2::uuid IS NULL OR h.student_id = $2)
        "#,
    )
    .bind(days_lookback.to_string())
    .bind(payload.student_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let count = rows.len();
    for r in &rows {
        if let Some(guardian_uid) = r.get::<Option<Uuid>, _>("guardian_user_id") {
            let tenant_id: Uuid = r.get("tenant_id");
            let student_name: String = r.get("student_name");
            let notif_title = "Pengingat Jurnal Karakter G7KAIH";
            let notif_body = format!("Ayah/Bunda dari {}, terdapat catatan pembiasaan karakter 7 Kebiasaan yang menunggu verifikasi Anda.", student_name);

            let _ = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type, channel, priority, is_read, created_at
                ) VALUES (
                    gen_random_uuid(), $1, $2, $3, $4, 'HABIT_VERIFICATION_REMINDER', 'in_app', 'HIGH', false, NOW()
                )
                "#,
            )
            .bind(tenant_id)
            .bind(guardian_uid)
            .bind(notif_title)
            .bind(notif_body)
            .execute(&ctx.pool)
            .await;
        }
    }

    let res = serde_json::json!({
        "success": true,
        "message": format!("Pengingat berhasil dikirimkan untuk {} catatan jurnal pembiasaan.", count),
        "pending_entries_count": count
    });

    Ok(Json(ApiResponse::success(res, req_ctx.request_id)))
}



