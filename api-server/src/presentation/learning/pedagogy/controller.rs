use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;
use sqlx::Row;

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
