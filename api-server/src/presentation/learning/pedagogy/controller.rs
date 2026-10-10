use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, DomainError};

use super::dto::{
    CpRegistryLookupResponse, CpSearchQuery, LearningOutcomeElementDto,
    ProposedTpDto, RegisterSchoolCpPayload, SynthesizeCpRequest, SynthesizeCpResponse,
};

pub fn pedagogy_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/cp", get(lookup_cp))
        .route("/cp/school", post(register_school_cp))
        .route("/cp/synthesize", post(synthesize_cp))
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
    let rows = sqlx::query!(
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
        req_ctx.tenant_id,
        phase,
        subject_pattern,
        subject_raw,
        verification_filter
    )
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
            let is_eligible = r.verification_status == "NATIONAL_VERIFIED"
                || r.verification_status == "SCHOOL_VERIFIED";

            LearningOutcomeElementDto {
                id: r.id,
                tenant_id: r.tenant_id,
                subject_code: r.subject_code,
                subject_name: r.subject_name,
                phase: r.phase,
                target_grades: r.target_grades,
                element_name: r.element_name,
                element_code: r.element_code,
                description: r.description,
                source_origin: r.source_origin,
                source_version: r.source_version,
                source_document: r.source_document,
                document_page_ref: r.document_page_ref,
                verification_status: r.verification_status,
                is_eligible_source: is_eligible,
                order_index: r.order_index,
                created_at: r.created_at,
                updated_at: r.updated_at,
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

    let initial_audit = serde_json::json!([
        {
            "action": "SCHOOL_REGISTRATION",
            "actor_id": req_ctx.actor.id,
            "timestamp": chrono::Utc::now(),
            "notes": "Didaftarkan langsung oleh satuan pendidikan sebagai bagian dari KOSP sekolah"
        }
    ]);

    let row = sqlx::query!(
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
        req_ctx.tenant_id,
        payload.subject_code.as_deref(),
        subject_name,
        phase,
        payload.target_grades.trim(),
        element_name,
        payload.element_code.as_deref(),
        description,
        payload.source_document.as_deref().unwrap_or("Dokumen KOSP Satuan Pendidikan"),
        payload.document_page_ref.as_deref(),
        req_ctx.actor.id,
        initial_audit,
        payload.order_index.unwrap_or(0)
    )
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
        id: row.id,
        tenant_id: row.tenant_id,
        subject_code: row.subject_code,
        subject_name: row.subject_name,
        phase: row.phase,
        target_grades: row.target_grades,
        element_name: row.element_name,
        element_code: row.element_code,
        description: row.description,
        source_origin: row.source_origin,
        source_version: row.source_version,
        source_document: row.source_document,
        document_page_ref: row.document_page_ref,
        verification_status: row.verification_status,
        is_eligible_source: true,
        order_index: row.order_index,
        created_at: row.created_at,
        updated_at: row.updated_at,
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
    let cp_row = sqlx::query!(
        r#"
        SELECT 
            id, tenant_id, subject_code, subject_name, phase, target_grades,
            element_name, element_code, description, source_origin, source_version,
            verification_status
        FROM learning_outcomes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        payload.source_cp_id
    )
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

    let is_eligible = cp_row.verification_status == "NATIONAL_VERIFIED"
        || cp_row.verification_status == "SCHOOL_VERIFIED";

    if !is_eligible {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Capaian Pembelajaran '{}' berstatus '{}'. Hanya CP berstatus NATIONAL_VERIFIED atau SCHOOL_VERIFIED yang boleh dijadikan basis dekonstruksi AI.",
                cp_row.element_name, cp_row.verification_status
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
        tenant_id_str, cp_row.id, cp_row.source_version, grade_level, academic_year, model_name, prompt_version
    );
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(cache_seed.as_bytes());
    let cache_key = hex::encode(hasher.finalize());

    if !force_regenerate {
        let cached = sqlx::query!(
            r#"
            SELECT id, status FROM curriculum_ai_cache
            WHERE cache_key = $1 AND status = 'DRAFT_GENERATED'
            "#,
            cache_key
        )
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
            let existing_tps = sqlx::query!(
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
                cp_row.id,
                req_ctx.tenant_id
            )
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
                let current_version = existing_tps[0].version;
                let mapped_tps: Vec<ProposedTpDto> = existing_tps
                    .into_iter()
                    .map(|r| ProposedTpDto {
                        id: r.id,
                        code: r.code,
                        competency: r.competency.unwrap_or_else(|| "Memahami".to_string()),
                        bloom_level: r.bloom_level.unwrap_or_else(|| "C3".to_string()),
                        content_scope: r.content_scope,
                        statement: r.statement,
                        pancasila_profiles: r.pancasila_profiles.unwrap_or_default(),
                        evidence_indicators: r.evidence_indicators.unwrap_or_default(),
                        estimated_hours: r.estimated_hours,
                        publication_status: r.publication_status,
                        version: r.version,
                        semester: r.semester.unwrap_or_else(|| "ODD".to_string()),
                        sequence_order: r.sequence_order.unwrap_or(1),
                    })
                    .collect();

                return Ok(Json(ApiResponse::success(
                    SynthesizeCpResponse {
                        cache_hit: true,
                        source_cp_id: cp_row.id,
                        source_cp_element: cp_row.element_name,
                        source_verification_status: cp_row.verification_status,
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
        subject_name = cp_row.subject_name,
        phase = cp_row.phase,
        grade_level = grade_level,
        element_name = cp_row.element_name,
        description = cp_row.description,
        code_prefix = cp_row.subject_name.chars().filter(|c| c.is_alphabetic()).take(4).collect::<String>().to_uppercase(),
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
    let ver_row = sqlx::query!(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1 AS next_version
        FROM learning_objectives
        WHERE learning_outcome_id = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
        cp_row.id,
        req_ctx.tenant_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    let next_version = ver_row.next_version.unwrap_or(1);

    // Tandai draf lama sebagai is_superseded = true
    sqlx::query!(
        r#"
        UPDATE learning_objectives
        SET is_superseded = true, updated_at = NOW()
        WHERE learning_outcome_id = $1
          AND publication_status = 'DRAFT'
          AND is_superseded = false
          AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
        cp_row.id,
        req_ctx.tenant_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id,
        )
    })?;

    sqlx::query!(
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
        cp_row.id,
        req_ctx.tenant_id
    )
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
        "source_cp_id": cp_row.id,
        "source_cp_version": cp_row.source_version,
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

        let tp_row = sqlx::query!(
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
            req_ctx.tenant_id,
            cp_row.id,
            code,
            competency,
            bloom_level,
            content_scope,
            statement,
            &profiles,
            &indicators,
            hours,
            (idx + 1) as i32,
            next_version,
            ai_meta
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        let atp_row = sqlx::query!(
            r#"
            INSERT INTO learning_objective_flows (
                tenant_id, learning_objective_id, academic_year, grade_level, semester,
                sequence_order, allocated_hours, pedagogical_approach, publication_status, version, is_superseded
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, 'DRAFT', $9, false
            )
            RETURNING id, semester, sequence_order, allocated_hours
            "#,
            req_ctx.tenant_id,
            tp_row.id,
            academic_year,
            grade_level,
            semester_clean,
            seq,
            hours,
            approach,
            next_version
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id,
            )
        })?;

        proposed_tps.push(ProposedTpDto {
            id: tp_row.id,
            code: tp_row.code,
            competency: tp_row.competency.unwrap_or_default(),
            bloom_level: tp_row.bloom_level.unwrap_or_default(),
            content_scope: tp_row.content_scope,
            statement: tp_row.statement,
            pancasila_profiles: tp_row.pancasila_profiles.unwrap_or_default(),
            evidence_indicators: tp_row.evidence_indicators.unwrap_or_default(),
            estimated_hours: tp_row.estimated_hours,
            publication_status: tp_row.publication_status,
            version: tp_row.version,
            semester: atp_row.semester,
            sequence_order: atp_row.sequence_order,
        });
    }

    // Simpan entri cache
    let raw_val: serde_json::Value = serde_json::from_str(cleaned_json).unwrap_or(serde_json::Value::Null);
    sqlx::query!(
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
        req_ctx.tenant_id,
        cache_key,
        cp_row.id,
        cp_row.source_version,
        cp_row.phase,
        grade_level,
        academic_year,
        model_name,
        prompt_version,
        raw_val
    )
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

    Ok(Json(ApiResponse::success(
        SynthesizeCpResponse {
            cache_hit: false,
            source_cp_id: cp_row.id,
            source_cp_element: cp_row.element_name,
            source_verification_status: cp_row.verification_status,
            grade_level: grade_level.to_string(),
            academic_year: academic_year.to_string(),
            version: next_version,
            proposed_tps_count: proposed_tps.len(),
            proposed_tps,
            message: format!(
                "Berhasil mendekonstruksi CP '{}' menjadi {} butir usulan TP & ATP (Status: DRAFT v{}).",
                cp_row.element_name,
                parsed.tps.len(),
                next_version
            ),
        },
        req_ctx.request_id,
    )))
}
