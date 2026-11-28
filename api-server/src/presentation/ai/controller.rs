use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::convert::Infallible;
use tokio::sync::OnceCell;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    infrastructure::nvidia_ai::{
        self, GeneratedArticleDto, GeneratedAssignmentDto, GeneratedCbtQuizDto,
        GeneratedInfographicDto, OwnedChatMessage, StudentAnalyticsInsightDto,
    },
    response::ApiResponse,
};
use school_core::common::{
    error::{ApplicationError, DomainError},
    error_code::ErrorCode,
};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GenerateAiContentRequest {
    pub mode: String, // "INFOGRAPHIC" | "ARTICLE" | "ASSIGNMENT" | "QUIZ"
    pub topic: String,
    pub grade_level: Option<String>,
    pub subject_name: Option<String>,
    pub num_questions: Option<usize>,
    pub num_mcq: Option<usize>,
    pub num_essay: Option<usize>,
    pub difficulty: Option<String>,
    pub assignment_format: Option<String>,
    pub quiz_format: Option<String>,
    pub include_images: Option<bool>,
    #[serde(default)]
    pub question_types: Option<Vec<String>>,
    #[serde(default)]
    pub type_counts: Option<std::collections::HashMap<String, usize>>,
    #[serde(default)]
    pub exam_scope: Option<String>,
    #[serde(default)]
    pub period_label: Option<String>,
    #[serde(default)]
    pub source_materials: Option<Vec<String>>,
    #[serde(default)]
    pub material_ids: Option<Vec<Uuid>>,
}


#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GenerateAiContentResponse {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infographic: Option<GeneratedInfographicDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article: Option<GeneratedArticleDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment: Option<GeneratedAssignmentDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiz: Option<GeneratedCbtQuizDto>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GenerateAiImageRequest {
    pub prompt: String,
    pub subject_name: Option<String>,
    pub grade_level: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GenerateAiImageResponse {
    pub url: String,
    pub prompt: String,
    pub model: String,
    pub source: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ParseQuizDocumentRequest {
    pub raw_text: String,
    pub file_name: Option<String>,
    pub subject_name: Option<String>,
    pub grade_level: Option<String>,
}

async fn parse_quiz_document(
    req_ctx: RequestContext,
    Json(payload): Json<ParseQuizDocumentRequest>,
) -> Result<Json<ApiResponse<GeneratedCbtQuizDto>>, ApiError> {
    if payload.raw_text.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Teks hasil ekstraksi dokumen tidak boleh kosong".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let quiz = nvidia_ai::parse_quiz_document_from_text(
        &payload.raw_text,
        payload.file_name.as_deref(),
        payload.subject_name.as_deref(),
        payload.grade_level.as_deref(),
    )
    .await
    .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(quiz, req_ctx.request_id)))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AnalyzeStudentRequest {
    pub student_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct TanyaAiMessageDto {
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct TanyaAiRequest {
    pub question: String,
    pub subject_name: Option<String>,
    pub grade_level: Option<String>,
    pub history: Option<Vec<TanyaAiMessageDto>>,
    pub context: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TanyaAiResponse {
    pub reply: String,
    pub model: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AiFeedbackRequest {
    pub thread_id: String,
    pub message_id: String,
    pub rating: String, // "UP" | "DOWN"
    pub question_text: Option<String>,
    pub ai_reply_text: String,
    pub subject_name: Option<String>,
    pub grade_level: Option<String>,
    pub model_name: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AiFeedbackResponse {
    pub id: Uuid,
    pub rating: String,
    pub recorded: bool,
}

pub fn ai_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/generate-content", post(generate_content))
        .route("/generate-image", post(generate_image))
        .route("/parse-quiz-document", post(parse_quiz_document))
        .route("/analytics/student", post(analyze_student))
        .route("/tanya-ai", post(tanya_ai))
        .route("/tanya-ai/stream", post(tanya_ai_stream))
        .route("/feedback", post(submit_ai_feedback))
}

#[utoipa::path(
    post,
    path = "/api/v1/ai/generate-image",
    request_body = GenerateAiImageRequest,
    responses(
        (status = 200, description = "Generated educational image with FLUX AI successfully", body = inline(ApiResponse<GenerateAiImageResponse>)),
        (status = 400, description = "Validation error")
    ),
    security(("Bearer" = []))
)]
async fn generate_image(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<GenerateAiImageRequest>,
) -> Result<Json<ApiResponse<GenerateAiImageResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningQuizCreate)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::LearningQuizUpdate))
        .or_else(|_| require_permission(&req_ctx.actor, Permission::LearningMaterialCreate))
        .map_err(|_| {
            ApiError::new(
                ApplicationError::Unauthorized(
                    ErrorCode::AuthPermissionDenied,
                    "Hanya guru atau staf akademik yang dapat membuat gambar soal AI".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let prompt = payload.prompt.trim();
    if prompt.chars().count() < 3 {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Deskripsi gambar minimal 3 karakter".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let subject = payload.subject_name.as_deref().unwrap_or("Umum");
    let grade = payload.grade_level.as_deref().unwrap_or("Kurikulum Merdeka");

    let (url, model) = crate::infrastructure::cloudflare_ai::generate_quiz_question_image(
        &ctx.pool,
        prompt,
        subject,
        grade,
        payload.style.as_deref(),
    )
    .await
    .map_err(|err| {
        ApiError::new(
            ApplicationError::Internal(format!("Gagal generate gambar FLUX AI: {err}")),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(
        GenerateAiImageResponse {
            url,
            prompt: prompt.to_string(),
            model,
            source: "flux-ai".to_string(),
        },
        req_ctx.request_id,
    )))
}

async fn fetch_grounded_material_context_for_generation(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    subject: &str,
    clean_topic: &str,
    material_ids: Option<&[Uuid]>,
    source_materials: Option<&[String]>,
    exam_scope: Option<&str>,
    period_label: Option<&str>,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(scope) = exam_scope.map(str::trim).filter(|s| !s.is_empty()) {
        lines.push(format!("Cakupan Evaluasi: {scope}"));
    }
    if let Some(period) = period_label.map(str::trim).filter(|p| !p.is_empty()) {
        lines.push(format!("Periode Materi: {period}"));
    }

    let mut mat_rows = if let Some(ids) = material_ids.filter(|v| !v.is_empty()) {
        sqlx::query(
            r#"
            SELECT m.title, COALESCE(m.description, '') AS description
            FROM learning_materials m
            WHERE m.tenant_id = $1
              AND m.deleted_at IS NULL
              AND m.id = ANY($2)
            ORDER BY m.updated_at DESC
            LIMIT 6
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    if mat_rows.is_empty() {
        if let Some(titles) = source_materials.filter(|t| !t.is_empty()) {
            let clean_titles: Vec<String> = titles
                .iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .take(8)
                .collect();
            if !clean_titles.is_empty() {
                mat_rows = sqlx::query(
                    r#"
                    SELECT m.title, COALESCE(m.description, '') AS description
                    FROM learning_materials m
                    WHERE m.tenant_id = $1
                      AND m.deleted_at IS NULL
                      AND m.title = ANY($2)
                    ORDER BY m.updated_at DESC
                    LIMIT 6
                    "#,
                )
                .bind(tenant_id)
                .bind(&clean_titles)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
            }
        }
    }

    if mat_rows.is_empty() {
        let keyword = clean_topic
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 4)
            .max_by_key(|w| w.len())
            .unwrap_or(subject);
        let subj_like = format!("%{}%", subject.trim());
        let kw_like = format!("%{}%", keyword.trim());
        mat_rows = sqlx::query(
            r#"
            SELECT m.title, COALESCE(m.description, '') AS description
            FROM learning_materials m
            LEFT JOIN subjects s ON s.id = m.subject_id
            WHERE m.tenant_id = $1
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND (m.title ILIKE $2 OR m.description ILIKE $2 OR s.name ILIKE $3)
            ORDER BY CASE WHEN m.title ILIKE $2 THEN 0 ELSE 1 END, m.updated_at DESC
            LIMIT 4
            "#,
        )
        .bind(tenant_id)
        .bind(&kw_like)
        .bind(&subj_like)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    }

    let mut total_excerpt_chars = 0usize;
    for r in mat_rows {
        if total_excerpt_chars >= 2000 {
            break;
        }
        let title: String = r.try_get("title").unwrap_or_default();
        let desc: String = r.try_get("description").unwrap_or_default();
        let compact_desc: String = desc
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(450)
            .collect();
        if !title.trim().is_empty() {
            let entry = if compact_desc.is_empty() {
                format!("- Judul Materi: {}", title.trim())
            } else {
                total_excerpt_chars += compact_desc.chars().count();
                format!("- Judul Materi: {} | Ringkasan Konten: {}", title.trim(), compact_desc)
            };
            lines.push(entry);
        }
    }

    if let Some(titles) = source_materials.filter(|t| !t.is_empty()) {
        if total_excerpt_chars == 0 {
            let joined = titles
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .take(10)
                .collect::<Vec<_>>()
                .join("; ");
            if !joined.is_empty() {
                lines.push(format!("- Topik Materi Terpilih: {joined}"));
            }
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/ai/generate-content",
    request_body = GenerateAiContentRequest,
    responses(
        (status = 200, description = "Generated AI content successfully", body = inline(ApiResponse<GenerateAiContentResponse>)),
        (status = 400, description = "Validation error")
    ),
    security(("Bearer" = []))
)]
async fn generate_content(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<GenerateAiContentRequest>,
) -> Result<Json<ApiResponse<GenerateAiContentResponse>>, ApiError> {
    let topic = payload.topic.trim();
    if topic.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Topik materi pembelajaran wajib diisi".to_string())),
            &req_ctx.request_id,
        ));
    }

    let grade = payload.grade_level.as_deref().unwrap_or("Kelas 5 SD");
    let subject = payload.subject_name.as_deref().unwrap_or("Umum");
    let num_q = payload.num_questions.unwrap_or(5).clamp(1, 15);
    let diff = payload.difficulty.as_deref().unwrap_or("Sedang");
    let include_images = payload.include_images.unwrap_or(true);

    match payload.mode.to_uppercase().as_str() {
        "INFOGRAPHIC" => {
            let info = nvidia_ai::generate_infographic(&ctx.pool, topic, grade, subject, include_images)
                .await
                .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

            Ok(Json(ApiResponse::success(
                GenerateAiContentResponse {
                    mode: "INFOGRAPHIC".to_string(),
                    infographic: Some(info),
                    article: None,
                    assignment: None,
                    quiz: None,
                },
                req_ctx.request_id,
            )))
        }
        "ARTICLE" => {
            let art = nvidia_ai::generate_article(topic, grade, subject)
                .await
                .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

            Ok(Json(ApiResponse::success(
                GenerateAiContentResponse {
                    mode: "ARTICLE".to_string(),
                    infographic: None,
                    article: Some(art),
                    assignment: None,
                    quiz: None,
                },
                req_ctx.request_id,
            )))
        }
        "ASSIGNMENT" => {
            let format = payload.assignment_format.as_deref().unwrap_or("STRUCTURED_QUESTIONS");
            let fmt_upper = format.to_uppercase();
            let is_pure_game = fmt_upper == "INTERACTIVE_GAMES";
            let is_any_game = is_pure_game || fmt_upper.contains("GAME");
            let is_homework = fmt_upper == "HOMEWORK_PR"
                || (!is_any_game && (fmt_upper.contains("HOMEWORK") || fmt_upper.contains("PR")));

            let mut num_mcq = if is_homework || is_pure_game {
                0
            } else {
                payload.num_mcq
                    .or_else(|| payload.num_questions.map(|q| (q * 7 / 10).max(1)))
                    .unwrap_or(4)
                    .clamp(0, 25)
            };

            let num_essay = if is_any_game {
                0
            } else if is_homework {
                payload.num_essay
                    .or(payload.num_questions)
                    .unwrap_or(2)
                    .clamp(1, 15)
            } else {
                payload.num_essay
                    .or_else(|| payload.num_questions.map(|q| q.saturating_sub(num_mcq).max(1)))
                    .unwrap_or(2)
                    .clamp(0, 15)
            };

            if !is_homework && !is_any_game && num_mcq == 0 && num_essay == 0 {
                num_mcq = 4;
            }

            let include_images = payload.include_images.unwrap_or(false);
            let (clean_topic, _extracted_scope) = nvidia_ai::extract_clean_assessment_topic(topic);
            let material_context = fetch_grounded_material_context_for_generation(
                &ctx.pool,
                req_ctx.tenant_id,
                subject,
                &clean_topic,
                payload.material_ids.as_deref(),
                payload.source_materials.as_deref(),
                payload.exam_scope.as_deref(),
                payload.period_label.as_deref(),
            )
            .await;

            let task = nvidia_ai::generate_assignment(
                &ctx.pool,
                &clean_topic,
                grade,
                subject,
                Some(format),
                num_mcq,
                num_essay,
                diff,
                include_images,
                material_context.as_deref(),
            )
            .await
            .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

            Ok(Json(ApiResponse::success(
                GenerateAiContentResponse {
                    mode: "ASSIGNMENT".to_string(),
                    infographic: None,
                    article: None,
                    assignment: Some(task),
                    quiz: None,
                },
                req_ctx.request_id,
            )))
        }
        "QUIZ" => {
            let quiz_format = payload.quiz_format.as_deref();
            let requested_types = payload.question_types.as_deref();
            let has_variatif_types = requested_types
                .map(|ts| {
                    ts.iter().any(|t| {
                        let u = t.trim().to_uppercase();
                        u != "MULTIPLE_CHOICE" && u != "PG" && u != "ESSAY" && u != "URAIAN"
                    })
                })
                .unwrap_or(false)
                || quiz_format
                    .map(|f| f.to_uppercase().contains("VARIATIF"))
                    .unwrap_or(false);
            let is_mixed = has_variatif_types
                || quiz_format
                    .map(|f| f.to_uppercase() == "MIXED" || f.to_uppercase().contains("CAMPURAN"))
                    .unwrap_or_else(|| payload.num_essay.map(|e| e > 0).unwrap_or(false));
            let num_mcq = payload.num_mcq
                .or_else(|| {
                    if is_mixed {
                        payload.num_questions.map(|q| (q * 7 / 10).max(1))
                    } else {
                        payload.num_questions
                    }
                })
                .unwrap_or(if is_mixed { 10 } else { num_q })
                .clamp(0, 50);
            let num_essay = if is_mixed {
                payload.num_essay
                    .or_else(|| payload.num_questions.map(|q| q.saturating_sub(num_mcq)))
                    .unwrap_or(2)
                    .clamp(0, 25)
            } else {
                0
            };
            let include_images = payload.include_images.unwrap_or(false);
            let (clean_topic, extracted_scope) = nvidia_ai::extract_clean_assessment_topic(topic);
            let effective_scope = payload
                .exam_scope
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or(extracted_scope.as_deref());
            let material_context = fetch_grounded_material_context_for_generation(
                &ctx.pool,
                req_ctx.tenant_id,
                subject,
                &clean_topic,
                payload.material_ids.as_deref(),
                payload.source_materials.as_deref(),
                effective_scope,
                payload.period_label.as_deref(),
            )
            .await;

            let quiz = nvidia_ai::generate_cbt_quiz(
                &ctx.pool,
                &clean_topic,
                grade,
                subject,
                quiz_format,
                requested_types,
                payload.type_counts.as_ref(),
                num_mcq,
                num_essay,
                diff,
                include_images,
                material_context.as_deref(),
            )
            .await
            .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

            Ok(Json(ApiResponse::success(
                GenerateAiContentResponse {
                    mode: "QUIZ".to_string(),
                    infographic: None,
                    article: None,
                    assignment: None,
                    quiz: Some(quiz),
                },
                req_ctx.request_id,
            )))
        }

        other => Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(format!(
                "Mode '{other}' tidak didukung. Pilihan: INFOGRAPHIC, ARTICLE, ASSIGNMENT, QUIZ"
            ))),
            &req_ctx.request_id,
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/ai/analytics/student",
    request_body = AnalyzeStudentRequest,
    responses(
        (status = 200, description = "Generated student AI analytics successfully", body = inline(ApiResponse<StudentAnalyticsInsightDto>)),
        (status = 404, description = "Student not found")
    ),
    security(("Bearer" = []))
)]
async fn analyze_student(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<AnalyzeStudentRequest>,
) -> Result<Json<ApiResponse<StudentAnalyticsInsightDto>>, ApiError> {
    // 1. Fetch student basic info
    let student_row = sqlx::query(
        r#"
        SELECT s.id, s.full_name as name, COALESCE(c.name, 'Kelas Umum') as class_name
        FROM students s
        LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'ACTIVE' OR e.status = 'Active')
        LEFT JOIN classes c ON c.id = e.class_id
        WHERE s.id = $1 AND s.tenant_id = $2 AND s.deleted_at IS NULL
        LIMIT 1
        "#
    )
    .bind(payload.student_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::from(e), &req_ctx.request_id))?
    .ok_or_else(|| {
        ApiError::new(
            ApplicationError::NotFound(
                ErrorCode::StudentNotFound,
                "Siswa tidak ditemukan".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_name: String = student_row.get("name");
    let class_name: String = student_row.get("class_name");

    // 2. Compute attendance rate
    let attendance_row = sqlx::query(
        r#"
        SELECT 
            COUNT(*) as total_days,
            COUNT(*) FILTER (WHERE UPPER(status) = 'PRESENT' OR UPPER(status) = 'HADIR') as present_days
        FROM session_attendances
        WHERE student_id = $1 AND tenant_id = $2
        "#
    )
    .bind(payload.student_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let (total_days, present_days): (i64, i64) = attendance_row
        .map(|r| (r.get("total_days"), r.get("present_days")))
        .unwrap_or((0, 0));

    let attendance_pct = if total_days > 0 {
        (present_days as f64 / total_days as f64) * 100.0
    } else {
        90.0 // Default healthy benchmark if no attendance logged yet
    };

    // 3. Compute quiz average score
    let score_row = sqlx::query(
        r#"
        SELECT COALESCE(AVG(score), 75.0) as avg_score
        FROM quiz_attempts
        WHERE student_id = $1 AND tenant_id = $2 AND status IN ('submitted', 'graded')
        "#
    )
    .bind(payload.student_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let avg_score: f64 = score_row.map(|r| r.get("avg_score")).unwrap_or(75.0);

    // 4. Compute uncompleted tasks
    let uncompleted_row = sqlx::query(
        r#"
        SELECT COUNT(*) as uncompleted_count
        FROM student_material_completions
        WHERE student_id = $1 AND tenant_id = $2 AND is_completed = false
        "#
    )
    .bind(payload.student_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let uncompleted_tasks: i64 = uncompleted_row.map(|r| r.get("uncompleted_count")).unwrap_or(0);

    // 5. Generate AI diagnosis using NVIDIA NIM
    let insights = nvidia_ai::generate_student_analytics(
        payload.student_id,
        &student_name,
        &class_name,
        attendance_pct,
        avg_score,
        uncompleted_tasks,
    )
    .await
    .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(insights, req_ctx.request_id)))
}

struct PreparedTanyaAiContext {
    messages: Vec<OwnedChatMessage>,
    subject: String,
    question: String,
    fallback_model: String,
}

async fn fetch_grounded_curriculum_sources(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    question: &str,
    subject: &str,
    grade: &str,
) -> Vec<String> {
    let mut sources: Vec<String> = Vec::new();
    let keyword_term = question
        .split_whitespace()
        .find(|w| w.len() >= 4 && !w.starts_with('['))
        .unwrap_or(subject);
    let subject_like = format!("%{}%", subject.trim());
    let keyword_like = format!("%{}%", keyword_term.trim());

    // 1. Real SIBI / Digital Library Books matching subject or question keyword
    if let Ok(book_rows) = sqlx::query(
        r#"
        SELECT lb.title,
               COALESCE(lb.publisher, 'SIBI Kemdikdasmen') AS publisher,
               lb.total_pages,
               COALESCE(s.name, '') AS subject_name
        FROM library_books lb
        LEFT JOIN subjects s ON s.id = lb.subject_id
        WHERE (lb.title ILIKE $1 OR s.name ILIKE $1 OR lb.title ILIKE $2)
        ORDER BY CASE WHEN lb.title ILIKE $2 THEN 0 ELSE 1 END, lb.created_at DESC
        LIMIT 3
        "#,
    )
    .bind(&subject_like)
    .bind(&keyword_like)
    .fetch_all(pool)
    .await
    {
        for r in book_rows {
            let title: String = r.try_get("title").unwrap_or_default();
            let publisher: String = r.try_get("publisher").unwrap_or_else(|_| "SIBI Kemdikdasmen".to_string());
            let total_pages: i32 = r.try_get("total_pages").unwrap_or(0);
            let subj: String = r.try_get("subject_name").unwrap_or_default();
            if !title.is_empty() {
                sources.push(format!(
                    "Buku SIBI \"{title}\" ({publisher}{}{})",
                    if subj.is_empty() { String::new() } else { format!(", Mapel {subj}") },
                    if total_pages > 0 { format!(", {total_pages} hal.") } else { String::new() }
                ));
            }
        }
    }

    // 2. Real Teacher Learning Materials in this tenant
    if let Ok(mat_rows) = sqlx::query(
        r#"
        SELECT lm.title,
               COALESCE(t.full_name, '') AS teacher_name,
               lm.start_page,
               lm.end_page
        FROM learning_materials lm
        LEFT JOIN teachers t ON t.id = lm.teacher_id
        LEFT JOIN subjects s ON s.id = lm.subject_id
        WHERE lm.tenant_id = $1
          AND lm.deleted_at IS NULL
          AND lm.is_active = true
          AND (lm.title ILIKE $2 OR s.name ILIKE $3 OR lm.description ILIKE $2)
        ORDER BY lm.updated_at DESC
        LIMIT 3
        "#,
    )
    .bind(tenant_id)
    .bind(&keyword_like)
    .bind(&subject_like)
    .fetch_all(pool)
    .await
    {
        for r in mat_rows {
            let title: String = r.try_get("title").unwrap_or_default();
            let teacher: String = r.try_get("teacher_name").unwrap_or_default();
            let sp: Option<i32> = r.try_get("start_page").ok().flatten();
            let ep: Option<i32> = r.try_get("end_page").ok().flatten();
            let page_info = match (sp, ep) {
                (Some(a), Some(b)) => format!(", Halaman {a}–{b}"),
                (Some(a), None) => format!(", Halaman {a}"),
                _ => String::new(),
            };
            let teacher_info = if teacher.is_empty() {
                String::new()
            } else {
                format!(" oleh {teacher}")
            };
            if !title.is_empty() {
                sources.push(format!("Materi Ajar \"{title}\"{teacher_info}{page_info}"));
            }
        }
    }

    // 3. Real Modul Ajar Kurikulum Merdeka authored by teachers
    if let Ok(modul_rows) = sqlx::query(
        r#"
        SELECT ma.title,
               ma.grade_level,
               ma.subject_name,
               COALESCE(u.full_name, '') AS author_name
        FROM modul_ajar ma
        LEFT JOIN users u ON u.id = ma.created_by
        WHERE (ma.tenant_id = $1 OR ma.tenant_id IS NULL)
          AND ma.deleted_at IS NULL
          AND (ma.subject_name ILIKE $2 OR ma.title ILIKE $3 OR ma.grade_level ILIKE $4)
        ORDER BY ma.updated_at DESC
        LIMIT 2
        "#,
    )
    .bind(tenant_id)
    .bind(&subject_like)
    .bind(&keyword_like)
    .bind(format!("%{}%", grade.trim()))
    .fetch_all(pool)
    .await
    {
        for r in modul_rows {
            let title: String = r.try_get("title").unwrap_or_default();
            let m_grade: String = r.try_get("grade_level").unwrap_or_default();
            let author: String = r.try_get("author_name").unwrap_or_default();
            if !title.is_empty() {
                let by_author = if author.is_empty() { String::new() } else { format!(" ({author})") };
                sources.push(format!("Modul Ajar \"{title}\" — {m_grade}{by_author}"));
            }
        }
    }

    sources
}

async fn fetch_principal_teacher_productivity_context(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Option<String> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(NULLIF(TRIM(t.full_name), ''), u.full_name, 'Guru') AS teacher_name,
            COALESCE(NULLIF(TRIM(t.subject), ''), '-') AS subject_name,
            COUNT(DISTINCT m.id) FILTER (WHERE m.deleted_at IS NULL)::bigint AS modules_count,
            COUNT(DISTINCT a.id) FILTER (WHERE a.deleted_at IS NULL)::bigint AS assignments_count,
            COUNT(DISTINCT q.id) FILTER (WHERE q.deleted_at IS NULL)::bigint AS quizzes_count
        FROM teachers t
        LEFT JOIN users u ON u.id = t.user_id
        LEFT JOIN learning_materials m ON m.tenant_id = t.tenant_id AND m.deleted_at IS NULL
            AND (
                m.teacher_id = t.id
                OR (t.user_id IS NOT NULL AND (m.teacher_id = t.user_id OR m.created_by = t.user_id))
            )
        LEFT JOIN assignments a ON a.tenant_id = t.tenant_id AND a.deleted_at IS NULL
            AND (
                a.teacher_id = t.id
                OR (t.user_id IS NOT NULL AND (a.teacher_id = t.user_id OR a.created_by = t.user_id))
            )
        LEFT JOIN quizzes q ON q.tenant_id = t.tenant_id AND q.deleted_at IS NULL
            AND (
                q.teacher_id = t.id
                OR (t.user_id IS NOT NULL AND (q.teacher_id = t.user_id OR q.created_by = t.user_id))
            )
        WHERE t.tenant_id = $1 AND t.deleted_at IS NULL
        GROUP BY
            t.id,
            COALESCE(NULLIF(TRIM(t.full_name), ''), u.full_name, 'Guru'),
            COALESCE(NULLIF(TRIM(t.subject), ''), '-')
        ORDER BY
            (
                COUNT(DISTINCT m.id) FILTER (WHERE m.deleted_at IS NULL)
                + COUNT(DISTINCT a.id) FILTER (WHERE a.deleted_at IS NULL)
                + COUNT(DISTINCT q.id) FILTER (WHERE q.deleted_at IS NULL)
            ) DESC,
            teacher_name ASC
        LIMIT 60
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .ok()?;

    if rows.is_empty() {
        return None;
    }

    let teacher_count = rows.len();
    let mut sum_modules: i64 = 0;
    let mut sum_assignments: i64 = 0;
    let mut sum_quizzes: i64 = 0;
    let mut entries: Vec<String> = Vec::with_capacity(teacher_count);

    for (idx, row) in rows.iter().enumerate() {
        let name: String = row
            .try_get("teacher_name")
            .unwrap_or_else(|_| "Guru".to_string());
        let subject: String = row
            .try_get("subject_name")
            .unwrap_or_else(|_| "-".to_string());
        let modules: i64 = row.try_get("modules_count").unwrap_or(0);
        let assignments: i64 = row.try_get("assignments_count").unwrap_or(0);
        let quizzes: i64 = row.try_get("quizzes_count").unwrap_or(0);
        let total = modules + assignments + quizzes;

        sum_modules += modules;
        sum_assignments += assignments;
        sum_quizzes += quizzes;

        entries.push(format!(
            "{}. {} ({}) — Modul: {}, Tugas: {}, Kuis CBT: {}, Total: {}",
            idx + 1,
            name,
            subject,
            modules,
            assignments,
            quizzes,
            total
        ));
    }

    let denom = teacher_count as f64;
    let avg_modules = (sum_modules as f64) / denom;
    let avg_assignments = (sum_assignments as f64) / denom;
    let avg_quizzes = (sum_quizzes as f64) / denom;

    Some(format!(
        "[DAFTAR KEAKTIFAN AKTUAL PER GURU DARI DATABASE ({teacher_count} Guru, Diurutkan Paling Aktif s.d. Terendah)]\n\
        Ringkasan Tepat: Total Guru Aktif = {teacher_count}, Total Modul Terkait Guru = {sum_modules} (rerata {avg_modules:.2} modul/guru), Total Tugas Terkait Guru = {sum_assignments} (rerata {avg_assignments:.2} tugas/guru), Total Kuis CBT Terkait Guru = {sum_quizzes} (rerata {avg_quizzes:.2} kuis/guru).\n\
        Daftar Nama Asli Guru & Statistik Aktual:\n\
        {}\n\
        ATURAN KETAT ANALISIS KEPALA SEKOLAH:\n\
        - WAJIB menyebut NAMA ASLI GURU dan angka aktual dari daftar di atas untuk Top 3, Kelompok Menengah, dan Bottom 3 (butuh intervensi).\n\
        - DILARANG KERAS memakai nama samaran ilustratif ('Guru A', 'Guru B', 'Guru X', 'Guru Y', 'Guru Z'), menulis 'Data bersifat ilustratif', atau menyuruh menghubungi koordinator akademik.\n\
        - DILARANG KERAS mengarang angka estimasi/korelasi memakai '~' (seperti '~45 siswa', '~80 siswa', '~41 siswa', atau 'sekitar 41 siswa') untuk mengaitkan guru Top/Bottom 3 dengan siswa at-risk tanpa data pemetaan per kelas/mapel yang nyata di konteks.\n\
        - Jaga konsistensi angka rasio/rerata tepat 2 desimal (misal {avg_modules:.2} modul/guru dan {avg_assignments:.2} tugas/guru) di seluruh tabel & narasi.",
        entries.join("\n")
    ))
}

async fn fetch_principal_at_risk_breakdown_context(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
) -> Option<String> {
    let (
        active_students_res,
        gradebook_failed_res,
        unsubmitted_assign_res,
        low_assign_res,
        unread_mat_res,
        absent_att_res,
        low_quiz_res,
        by_class_res,
        by_subject_res,
    ) = tokio::join!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND status IN ('Active', 'active') AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT student_id) FROM gradebooks WHERE tenant_id = $1 AND passed = false",
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT s.id)::bigint
            FROM assignments a
            JOIN classes c ON c.id = a.class_id AND c.deleted_at IS NULL
            JOIN enrollments en ON en.class_id = c.id AND (en.status ILIKE 'active' OR en.status IS NULL)
            JOIN students s ON s.id = en.student_id AND s.deleted_at IS NULL AND s.is_active = true
            LEFT JOIN assignment_submissions sub ON sub.assignment_id = a.id
                AND (sub.student_id = s.id OR (s.user_id IS NOT NULL AND sub.student_id = s.user_id))
            WHERE a.tenant_id = $1
              AND a.deleted_at IS NULL
              AND a.is_active = true
              AND (a.status = 'published' OR a.status = 'active')
              AND (a.release_at IS NULL OR a.release_at <= NOW())
              AND (sub.id IS NULL OR sub.status = 'unsubmitted' OR sub.submitted_at IS NULL)
            "#,
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT s.id)::bigint
            FROM assignment_submissions sub
            JOIN assignments a ON a.id = sub.assignment_id
            JOIN students s ON (s.id = sub.student_id OR (s.user_id IS NOT NULL AND s.user_id = sub.student_id))
                AND s.deleted_at IS NULL
            WHERE sub.tenant_id = $1
              AND a.deleted_at IS NULL
              AND sub.score IS NOT NULL
              AND sub.score < 70
              AND sub.status != 'unsubmitted'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT s.id)::bigint
            FROM learning_materials m
            JOIN classes c ON c.id = m.class_id AND c.deleted_at IS NULL
            JOIN enrollments en ON en.class_id = c.id AND (en.status ILIKE 'active' OR en.status IS NULL)
            JOIN students s ON s.id = en.student_id AND s.deleted_at IS NULL AND s.is_active = true
            LEFT JOIN student_material_completions smc ON smc.student_id = s.id AND smc.material_id = m.id
            LEFT JOIN reading_progress rp ON rp.student_id = s.id AND rp.material_id = m.id
            WHERE m.tenant_id = $1
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND (m.visibility = 'published' OR m.visibility = 'active')
              AND (m.release_at IS NULL OR m.release_at <= NOW())
              AND smc.id IS NULL
              AND (rp.is_completed IS NULL OR rp.is_completed = false)
              AND COALESCE(rp.current_page, 1) < 3
            "#,
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT student_id)::bigint FROM session_attendances WHERE tenant_id = $1 AND status = 'absent'",
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT student_id)::bigint FROM quiz_attempts WHERE tenant_id = $1 AND score IS NOT NULL AND score < 70",
        )
        .bind(tenant_id)
        .fetch_one(pool),
        sqlx::query(
            r#"
            SELECT COALESCE(NULLIF(TRIM(c.name), ''), 'Kelas') AS class_name, COUNT(DISTINCT g.student_id)::bigint AS cnt
            FROM gradebooks g
            JOIN classes c ON c.id = g.class_id AND c.deleted_at IS NULL
            WHERE g.tenant_id = $1 AND g.passed = false
            GROUP BY c.name
            ORDER BY cnt DESC, c.name ASC
            LIMIT 15
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool),
        sqlx::query(
            r#"
            SELECT COALESCE(NULLIF(TRIM(s.name), ''), 'Mapel') AS subject_name, COUNT(DISTINCT g.student_id)::bigint AS cnt
            FROM gradebooks g
            JOIN subjects s ON s.id = g.subject_id AND s.deleted_at IS NULL
            WHERE g.tenant_id = $1 AND g.passed = false
            GROUP BY s.name
            ORDER BY cnt DESC, s.name ASC
            LIMIT 10
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool),
    );

    let active_students = active_students_res.unwrap_or(0);
    let gradebook_failed = gradebook_failed_res.unwrap_or(0);
    let unsubmitted_assign = unsubmitted_assign_res.unwrap_or(0);
    let low_assign = low_assign_res.unwrap_or(0);
    let unread_mat = unread_mat_res.unwrap_or(0);
    let absent_att = absent_att_res.unwrap_or(0);
    let low_quiz = low_quiz_res.unwrap_or(0);

    let class_breakdown: Vec<String> = by_class_res
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let cname: String = r
                .try_get("class_name")
                .unwrap_or_else(|_| "Kelas".to_string());
            let cnt: i64 = r.try_get("cnt").unwrap_or(0);
            format!("{cname}: {cnt} siswa")
        })
        .collect();

    let subject_breakdown: Vec<String> = by_subject_res
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let sname: String = r
                .try_get("subject_name")
                .unwrap_or_else(|_| "Mapel".to_string());
            let cnt: i64 = r.try_get("cnt").unwrap_or(0);
            format!("{sname}: {cnt} siswa")
        })
        .collect();

    let class_line = if class_breakdown.is_empty() {
        "Belum ada rincian gradebook tidak tuntas per kelas.".to_string()
    } else {
        class_breakdown.join(" | ")
    };
    let subject_line = if subject_breakdown.is_empty() {
        "Belum ada rincian gradebook tidak tuntas per mapel.".to_string()
    } else {
        subject_breakdown.join(" | ")
    };

    Some(format!(
        "[RINCIAN INDIKATOR SISWA BUTUH INTERVENSI (AT-RISK / EWS) AKTUAL DARI DATABASE]\n\
        - Populasi Siswa Aktif: {active_students} siswa\n\
        - Siswa Belum Tuntas KKTP Rapor (gradebooks.passed = false, metrik KPI Dashboard): {gradebook_failed} siswa unik\n\
        - Indikator Risiko Nyata Terukur per Metrik (siswa unik, dapat beririsan antar indikator):\n\
          1) Belum Mengumpulkan Tugas Aktif: {unsubmitted_assign} siswa\n\
          2) Nilai Tugas di Bawah KKTP (< 70): {low_assign} siswa\n\
          3) Materi Wajib Belum Dibaca (< halaman 3): {unread_mat} siswa\n\
          4) Skor Kuis/CBT di Bawah 70: {low_quiz} siswa\n\
          5) Catatan Ketidakhadiran (Alfa/Absent): {absent_att} siswa\n\
        - Sebaran Aktual Siswa Belum Tuntas Rapor Per Kelas: {class_line}\n\
        - Sebaran Aktual Siswa Belum Tuntas Rapor Per Mata Pelajaran: {subject_line}\n\
        GUARDRAIL WAJIB ANALISIS SISWA AT-RISK:\n\
        - HANYA kutip angka aktual di atas! DILARANG membuat tabel korelasi fiktif memakai tanda '~' (misal '~45 siswa', '~80 siswa', '~41 siswa') atau mengasumsikan pembagian angka guru Bottom 3 ke total {gradebook_failed} siswa at-risk.\n\
        - Klasifikasikan akar masalah berdasarkan indikator nyata sistem di atas (belum kumpul tugas, nilai < 70, belum baca materi, ketidakhadiran); DILARANG menebak-nebak alasan non-database seperti 'motivasi/rumah tangga' atau 'HP/internet pribadi' sebagai penyebab pasti.\n\
        - Gunakan diksi Indonesia baku: tulis 'produksi'/'penyusunan' (BUKAN 'produkkan'), dan hindari kata 'rumah tangga'."
    ))
}

fn normalize_ai_reply_typos(raw: &str) -> String {
    nvidia_ai::normalize_ai_indonesian_typos(raw)
}

async fn prepare_tanya_ai_context(
    pool: &sqlx::PgPool,
    req_ctx: &RequestContext,
    payload: &TanyaAiRequest,
) -> Result<PreparedTanyaAiContext, ApiError> {
    let question = payload.question.trim().to_string();
    if question.is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation("Pertanyaan untuk AI tidak boleh kosong".to_string())),
            &req_ctx.request_id,
        ));
    }

    // Directly query database for authoritative active academic year & school info
    let db_academic_year = sqlx::query_scalar::<_, String>(
        "SELECT name FROM academic_years WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1"
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "2026/2027".to_string());

    let db_semester = sqlx::query_scalar::<_, i32>(
        "SELECT current_semester FROM academic_years WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1"
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(1);

    let sem_label = if db_semester == 2 { "Genap" } else { "Ganjil" };

    let school_meta = sqlx::query(
        "SELECT name, COALESCE(NULLIF(TRIM(headmaster_name), ''), '') AS headmaster_name FROM schools WHERE tenant_id = $1 AND deleted_at IS NULL LIMIT 1"
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let db_school_name = school_meta
        .as_ref()
        .and_then(|r| r.try_get::<String, _>("name").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Sekolah Anda".to_string());

    let lower_school_name = db_school_name.to_ascii_lowercase();
    let is_pkbm_institution = lower_school_name.contains("pkbm")
        || lower_school_name.contains("kesetaraan")
        || lower_school_name.contains("paket ");

    let mut db_headmaster_name = school_meta
        .as_ref()
        .and_then(|r| r.try_get::<String, _>("headmaster_name").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    if db_headmaster_name.is_empty() {
        db_headmaster_name = sqlx::query_scalar::<_, String>(
            r#"
            SELECT u.full_name
            FROM users u
            JOIN user_roles ur ON ur.user_id = u.id
            JOIN roles r ON r.id = ur.role_id
            WHERE u.tenant_id = $1
              AND u.is_active = true
              AND (LOWER(r.name) LIKE '%kepala%' OR LOWER(r.name) LIKE '%principal%' OR LOWER(r.name) LIKE '%headmaster%')
            LIMIT 1
            "#,
        )
        .bind(req_ctx.tenant_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Kepala Sekolah / Pimpinan PKBM".to_string());
    }

    let actor_user_name = if let Some(ref actor) = req_ctx.actor {
        sqlx::query_scalar::<_, String>("SELECT full_name FROM users WHERE id = $1 LIMIT 1")
            .bind(actor.id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    } else {
        String::new()
    };
    let actor_roles = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Pengguna".to_string());

    let subject = payload.subject_name.as_deref().unwrap_or("Umum").to_string();
    let grade = payload.grade_level.as_deref().unwrap_or("Siswa").to_string();

    // Trust ONLY authenticated actor roles (never client-supplied grade_level/subject_name) for privileged school analytics
    let is_principal = req_ctx.actor.as_ref().is_some_and(|a| {
        a.roles.iter().any(|r| {
            let n = r.name.trim().to_ascii_lowercase();
            n.contains("kepala")
                || n.contains("principal")
                || n.contains("headmaster")
                || n.contains("admin")
                || n.contains("operator")
        })
    });

    let is_teacher = is_principal
        || req_ctx.actor.as_ref().is_some_and(|a| {
            a.roles.iter().any(|r| {
                let n = r.name.trim().to_ascii_lowercase();
                n.contains("guru") || n.contains("teacher") || n.contains("tutor") || n.contains("pamong")
            })
        });

    let grounded_sources = if !is_principal {
        fetch_grounded_curriculum_sources(pool, req_ctx.tenant_id, &question, &subject, &grade).await
    } else {
        Vec::new()
    };

    let (principal_teacher_breakdown, principal_at_risk_breakdown) = if is_principal {
        let (t_ctx, r_ctx) = tokio::join!(
            fetch_principal_teacher_productivity_context(pool, req_ctx.tenant_id),
            fetch_principal_at_risk_breakdown_context(pool, req_ctx.tenant_id)
        );
        (t_ctx.unwrap_or_default(), r_ctx.unwrap_or_default())
    } else {
        (String::new(), String::new())
    };

    let sources_block = if grounded_sources.is_empty() {
        format!(
            "Belum ada judul spesifik yang cocok langsung di katalog query ini; JANGAN mengarang nomor halaman buku fiktif atau nama penulis palsu! \
            Untuk sitasi di akhir jawaban, cantumkan acuan resmi: \"📚 **Sumber:** Kurikulum Merdeka ({subject} • {grade}) & Data Akademik {db_school_name} T.A {db_academic_year} (Semester {sem_label})\"."
        )
    } else {
        let joined = grounded_sources
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Daftar Referensi Otoritatif dari Database Sekolah/SIBI:\n{joined}\n\
            WAJIB: Sertakan bagian penutup di paling akhir jawaban berupa baris:\n\
            📚 **Sumber:** <sebutkan sumber paling relevan dari daftar di atas atau acuan Kurikulum Merdeka {subject} {grade}; DILARANG mengarang nomor halaman atau nama guru yang tidak ada di daftar>."
        )
    };

    let actor_identity_line = if actor_user_name.is_empty() {
        String::new()
    } else {
        format!("\n        - Pengguna Sesi Chat Saat Ini: {actor_user_name} (Peran: {actor_roles})")
    };

    let is_english = nvidia_ai::is_english_subject(&subject);
    let puebi_guardrail = if is_english {
        ""
    } else {
        nvidia_ai::build_indonesian_puebi_and_logic_guardrail(true)
    };

    let institution_context_rule = if is_pkbm_institution {
        format!(
            "- KONTEKS LEMBAGA KESETARAAN (PKBM): {db_school_name} adalah lembaga Pendidikan Kesetaraan (PKBM) penyelenggara Kurikulum Merdeka Program Kesetaraan. Apabila menyebut kelas atau fase pada dokumen CP/ATP/RPP/asesmen, WAJIB menuliskan jenjang kesetaraannya secara eksplisit, misalnya \"Paket A (Setara SD • Kelas IV)\", \"Paket B (Setara SMP • Kelas VII-IX)\", atau \"Paket C (Setara SMA • Kelas X-XII)\" serta menyebut \"warga belajar / peserta didik\" (JANGAN menulis \"Kelas IV SD\" polos tanpa keterangan kesetaraan Paket A!)."
        )
    } else {
        format!(
            "- KONTEKS LEMBAGA: {db_school_name} menjalankan Kurikulum Merdeka. Sesuaikan jenjang kelas/fase dengan satuan pendidikan secara konsisten."
        )
    };

    let authoritative_db_directive = format!(
        "[KONTEKS OTORITATIF DATABASE REALTIME & ANTI-HALUSINASI]\n\
        - Satuan Pendidikan: {db_school_name}\n\
        {institution_context_rule}\n\
        - Kepala Sekolah Resmi (Profil Sekolah): {db_headmaster_name}{actor_identity_line}\n\
        - ATURAN IDENTITAS PIMPINAN: Kepala Sekolah resmi {db_school_name} adalah {db_headmaster_name}. JANGAN pernah menyebut operator, staf, atau guru lain (termasuk {actor_user_name}) sebagai Kepala Sekolah apabila bukan {db_headmaster_name}!\n\
        - Tahun Ajaran Aktif Resmi di Database: T.A {db_academic_year} (Semester {sem_label})\n\
        - INSTRUKSI KETAT: Seluruh analisis, rekomendasi, dan jawaban WAJIB merujuk ke Tahun Ajaran resmi aktif di database yaitu T.A {db_academic_year} (Semester {sem_label}). DILARANG MENGGUNAKAN tahun kadaluarsa seperti 2024/2025.\n\
        - IDENTITAS PLATFORM & ISTILAH KURIKULUM: Nama resmi sistem/LMS ini adalah **LMS Akselerasi Edu** (DILARANG KERAS menulis `SchoolOS` atau `LMS SchoolOS`). Patuhi Kurikulum Merdeka sepenuhnya: gunakan **CP, TP, ATP, dan KKTP** (DILARANG mencampur istilah K-13 seperti `Kompetensi Dasar (KD)`, `KD-4`, `KD Inti`, atau `KKM`).\n\
        - BAHASA, KAIDAH PUEBI/EYD V, BATAS NILAI & FORMAT PDF BERSIH: Gunakan ejaan Bahasa Indonesia baku sesuai EYD V / PUEBI & KBBI tanpa typo ('Capaian Pembelajaran', 'Asesmen', 'Kerangka Pembelajaran', 'Nilai Tempat', 'Guru:', 'Produktivitas', 'Aktivitas', 'Kolaborasi', 'Efektivitas', 'Produksi', 'memengaruhi', 'mengubah', 'praktik', 'analisis', 'risiko'; JANGAN menulis 'Capai Pembelajaran', 'CP Capai', 'Assesmen', 'Skeletasi', 'Gurunya:', 'rumpun nilai', 'rangkuman nilai', atau 'produkkan'). Untuk batas KKTP gunakan pertidaksamaan tegas tanpa celah (`≥ 70` vs `< 70`). DILARANG menulis tag `<br>` mentah dalam sel tabel Markdown atau token LaTeX (`\\dots`, `\\ldots`, `$...$`); gunakan simbol Unicode bersih (`...`, `< , > , =`, `≥`, `≤`).\n\
        {puebi_guardrail}\n\
        - KEJUJURAN AKADEMIK & SITASI SUMBER: Jika ragu pada fakta spesifik, katakan terus terang jangan mengarang angka taksiran atau mengarang aturan tata bahasa. Akhiri setiap jawaban edukatif/analitis dengan bagian singkat:\n  📚 **Sumber:** ...\n\
        {sources_block}\n\
        {principal_teacher_breakdown}\n\
        {principal_at_risk_breakdown}"
    );
    let system_prompt = if is_principal {
        format!(
            "Anda adalah 'AI Analitik Kepala Sekolah' (Executive Academic & Operational Intelligence Advisor) di LMS Akselerasi Edu. \
            Peran Anda adalah Asisten Eksekutif Kepala Sekolah / Pimpinan PKBM berstandar Kurikulum Merdeka. \
            Fokus & Kemampuan Utama: \
            1. Menganalisis data akademik real-time, ketercapaian target KKTP (batas interval tertutup seperti ≥ 70 tuntas vs < 70 perlu intervensi), dan evaluasi hasil belajar peserta didik / warga belajar. \
            2. Memantau keaktifan guru/tutor dalam pengunggahan materi/modul pembelajaran, pembuatan tugas mandiri, serta penyelenggaraan kuis dan asesmen CBT. \
            3. Memberikan evaluasi teknis supervisi manajerial, deteksi dini siswa butuh intervensi (Early Warning System), serta rekomendasi taktis peningkatan mutu lembaga. \
            4. Menyelaraskan seluruh dokumen & analisis dengan Kurikulum Merdeka Kemendikdasmen RI tanpa mencampur istilah K-13 (seperti KD atau KKM) serta tanpa menyebut merek lama SchoolOS. \
            Gunakan Bahasa Indonesia formal-profesional yang lugas, analitis, taktis, berbasis data riil yang disertakan dalam konteks, terstruktur rapi dengan Markdown bersih (tanpa tag HTML `<br>` atau sintaks LaTeX mentah seperti `\\dots` / `$...$`), dan tutup dengan baris `📚 **Sumber:**` yang menyebutkan data/periode laporan aktual."
        )
    } else if is_teacher {
        if is_english {
            format!(
                "You are 'AI Pedagogical & Teacher Co-Pilot' in LMS Akselerasi Edu. \
                Subject focus: {subject}, Grade/Class: {grade}. \
                Because the subject is English ({subject}), respond predominantly in clear, academic, and communicative English (unless the teacher asks for a bilingual explanation), formatted neatly with clean Markdown (no raw `<br>` HTML tags), and always end with `📚 **Sources:**`."
            )
        } else {
            format!(
                "Anda adalah 'AI Pedagogik & Workstation Guru' (Teacher Pedagogical & Instructional Co-Pilot) di LMS Akselerasi Edu. \
                Mata Pelajaran fokus: {subject}, Jenjang/Kelas: {grade}. \
                Anda patuh penuh pada regulasi Kurikulum Merdeka (Kepmendikbudristek & BSKAP No. 032/H/KR/2024 & BSKAP No. 033/H/KR/2024) tanpa mencampur istilah K-13 (KD/KKM). \
                Fokus & Kemampuan Utama: \
                1. Membantu penyusunan Draf Modul Ajar Kurikulum Merdeka yang komprehensif (Capaian Pembelajaran/CP, Tujuan Pembelajaran/TP, Alur Tujuan Pembelajaran/ATP, Pemahaman Bermakna, Pertanyaan Pemantik, Rencana Pembelajaran Berdiferensiasi Konten-Proses-Produk, LKPD, serta Asesmen Formatif & Sumatif). \
                2. Merancang kisi-kisi dan butir soal HOTS (Higher Order Thinking Skills) yang logis serta selaras fakta narasi soal untuk kuis dan asesmen CBT beserta kunci dan rubrik penilaian. \
                3. Membantu merumuskan Kriteria Ketercapaian Tujuan Pembelajaran (KKTP) berbasis interval nilai tegas (misal `≥ 70` tuntas vs `< 70` perlu intervensi) atau rubrik deskriptif untuk tugas siswa. \
                4. Memberikan saran diferensiasi pembelajaran dan rencana tindak lanjut remedial/pengayaan berdasarkan kondisi siswa nyata di kelas. \
                Gunakan Bahasa Indonesia edukatif yang profesional sesuai EYD V/KBBI, ramah, membimbing tahap demi tahap, solutif, terstruktur rapi dengan Markdown bersih (tanpa `<br>`, `\\dots`, atau `$...$` mentah), dan selalu cantumkan `📚 **Sumber:**` di akhir jawaban."
            )
        }
    } else if is_english {
        format!(
            "You are 'Guru AI' (Smart & Interactive English Learning Assistant) in LMS Akselerasi Edu. \
            Subject focus: {subject}, Grade/Class: {grade}. \
            Because the subject is English ({subject}), teach and reply in clear, encouraging English suitable for {grade} (provide brief Indonesian glosses only if the student explicitly asks), explain clearly with Markdown examples (without raw `<br>` tags), and ALWAYS end with `📚 **Sources:**` citing the given references (Teaching Module, SIBI Book, or Kurikulum Merdeka)."
        )
    } else {
        format!(
            "Anda adalah 'Guru AI' (Asisten Belajar Pintar & Interaktif Kurikulum Merdeka) di LMS Akselerasi Edu. \
            Mata Pelajaran fokus: {subject}, Jenjang/Kelas: {grade}. \
            Tugas Anda: Membantu siswa dan guru menjawab pertanyaan, menjelaskan materi pelajaran dengan jelas, ramah, membimbing tahap demi tahap, memberikan rumus dan contoh relevan dengan simbol Unicode yang rapi (tanpa tag `<br>` atau LaTeX `\\dots` mentah), serta memotivasi belajar. \
            Gunakan Bahasa Indonesia baku sesuai EYD V, komunikatif, rapi dalam format Markdown, serta WAJIB menyertakan `📚 **Sumber:**` di bagian paling akhir jawaban berdasarkan referensi yang diberikan (Buku SIBI, Modul Ajar/Materi Guru, atau Kurikulum Merdeka)."
        )
    };

    let mut messages: Vec<OwnedChatMessage> = Vec::new();
    messages.push(OwnedChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    });
    messages.push(OwnedChatMessage {
        role: "system".to_string(),
        content: authoritative_db_directive,
    });

    if let Some(ref ctx_str) = payload.context {
        if !ctx_str.trim().is_empty() {
            messages.push(OwnedChatMessage {
                role: "system".to_string(),
                content: ctx_str.clone(),
            });
        }
    }

    let history_slice: &[TanyaAiMessageDto] = payload.history.as_deref().unwrap_or(&[]);
    let history_tail = if history_slice.len() > 6 {
        &history_slice[history_slice.len() - 6..]
    } else {
        history_slice
    };

    for h in history_tail {
        let role = match h.role.as_str() {
            "assistant" | "model" | "bot" => "assistant",
            _ => "user",
        };
        let content = h.content.replace("2024/2025", &db_academic_year);
        messages.push(OwnedChatMessage {
            role: role.to_string(),
            content,
        });
    }

    messages.push(OwnedChatMessage {
        role: "user".to_string(),
        content: question.clone(),
    });

    let fallback_model = std::env::var("NVIDIA_MODEL")
        .unwrap_or_else(|_| "nvidia/nemotron-3.5-lightning-30b-a3b".to_string());

    Ok(PreparedTanyaAiContext {
        messages,
        subject,
        question,
        fallback_model,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/ai/tanya-ai",
    request_body = TanyaAiRequest,
    responses(
        (status = 200, description = "Tanya AI response generated successfully", body = inline(ApiResponse<TanyaAiResponse>)),
        (status = 400, description = "Validation error")
    ),
    security(("Bearer" = []))
)]
async fn tanya_ai(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<TanyaAiRequest>,
) -> Result<Json<ApiResponse<TanyaAiResponse>>, ApiError> {
    let prepared = prepare_tanya_ai_context(&ctx.pool, &req_ctx, &payload).await?;
    let msg_refs: Vec<nvidia_ai::ChatMessage<'_>> = prepared
        .messages
        .iter()
        .map(|m| nvidia_ai::ChatMessage {
            role: m.role.as_str(),
            content: m.content.as_str(),
        })
        .collect();

    let raw_reply = match nvidia_ai::call_nvidia_nim(msg_refs, 0.35, 2200).await {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!("NVIDIA NIM call failed for tanya-ai: {err:?}");
            if nvidia_ai::is_english_subject(&prepared.subject) {
                format!(
                    "Hello! Thank you for your question about **{}**.\n\n\
                    Your question: *\"{}\"* \n\n\
                    The AI tutor service is currently synchronizing. Please try again in a moment or discuss this topic directly with your English teacher during class! Keep up the great work!",
                    prepared.subject, prepared.question
                )
            } else {
                format!(
                    "Halo! Terima kasih atas pertanyaannya mengenai **{}**.\n\n\
                    Pertanyaan Anda: *\"{}\"* \n\n\
                    Saat ini jaringan komputasi Guru AI sedang mengalami sinkronisasi model NVIDIA AI. Silakan coba kembali sesaat lagi atau diskusikan pertanyaan ini langsung dengan Guru Pengampu di sesi belajar ya! Tetap semangat belajar! 🚀",
                    prepared.subject, prepared.question
                )
            }
        }
    };
    let reply = normalize_ai_reply_typos(&raw_reply);

    Ok(Json(ApiResponse::success(
        TanyaAiResponse {
            reply,
            model: prepared.fallback_model,
        },
        req_ctx.request_id,
    )))
}

async fn tanya_ai_stream(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<TanyaAiRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let prepared = prepare_tanya_ai_context(&ctx.pool, &req_ctx, &payload).await?;
    let (sse_tx, sse_rx) = tokio::sync::mpsc::channel::<Event>(64);

    tokio::spawn(async move {
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::channel::<String>(64);
        let sse_tx_forward = sse_tx.clone();

        let forward_task = tokio::spawn(async move {
            let mut sent_any = false;
            while let Some(chunk) = delta_rx.recv().await {
                sent_any = true;
                let data = json!({ "text": chunk }).to_string();
                if sse_tx_forward
                    .send(Event::default().event("delta").data(data))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            sent_any
        });

        let stream_result = nvidia_ai::stream_nvidia_nim(prepared.messages, 0.35, 2200, delta_tx).await;
        let sent_any = forward_task.await.unwrap_or(false);

        let used_model = match stream_result {
            Ok(model) => model,
            Err(err) => {
                tracing::warn!("NVIDIA NIM SSE stream failed for tanya-ai: {err:?}");
                if !sent_any {
                    let fallback = if nvidia_ai::is_english_subject(&prepared.subject) {
                        format!(
                            "Hello! Thank you for your question about **{}**.\n\n\
                            Your question: *\"{}\"* \n\n\
                            The AI tutor service is currently synchronizing. Please try again shortly or discuss this question directly with your English teacher during class! Keep up the great work!",
                            prepared.subject, prepared.question
                        )
                    } else {
                        format!(
                            "Halo! Terima kasih atas pertanyaannya mengenai **{}**.\n\n\
                            Pertanyaan Anda: *\"{}\"* \n\n\
                            Saat ini jaringan komputasi Guru AI sedang mengalami sinkronisasi model NVIDIA AI. Silakan coba kembali sesaat lagi atau diskusikan langsung dengan Guru Pengampu di sesi belajar ya! Tetap semangat belajar! 🚀",
                            prepared.subject, prepared.question
                        )
                    };
                    let _ = sse_tx
                        .send(Event::default().event("delta").data(json!({ "text": fallback }).to_string()))
                        .await;
                }
                prepared.fallback_model
            }
        };

        let done_payload = json!({ "model": used_model, "finished": true }).to_string();
        let _ = sse_tx
            .send(Event::default().event("done").data(done_payload))
            .await;
    });

    let stream = ReceiverStream::new(sse_rx).map(Ok::<Event, Infallible>);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

static AI_FEEDBACK_TABLE_READY: OnceCell<()> = OnceCell::const_new();

async fn ensure_ai_feedback_table(_pool: &sqlx::PgPool) {
    // Schema managed exclusively via SQLx migrations (20261117180000_create_ai_feedback_logs.sql).
    AI_FEEDBACK_TABLE_READY.get_or_init(|| async {}).await;
}

async fn submit_ai_feedback(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<AiFeedbackRequest>,
) -> Result<Json<ApiResponse<AiFeedbackResponse>>, ApiError> {
    let normalized_rating = match payload.rating.trim().to_uppercase().as_str() {
        "UP" | "UPVOTE" | "THUMBS_UP" | "HELPFUL" | "GOOD" => "UP",
        "DOWN" | "DOWNVOTE" | "THUMBS_DOWN" | "INACCURATE" | "BAD" => "DOWN",
        _ => {
            return Err(ApiError::new(
                ApplicationError::Domain(DomainError::Validation(
                    "Rating feedback harus bernilai 'UP' atau 'DOWN'".to_string(),
                )),
                &req_ctx.request_id,
            ));
        }
    };

    let msg_id = payload.message_id.trim();
    let thread_id = payload.thread_id.trim();
    if msg_id.is_empty() || thread_id.is_empty() || payload.ai_reply_text.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "thread_id, message_id, dan ai_reply_text wajib diisi".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    ensure_ai_feedback_table(&ctx.pool).await;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_else(Uuid::nil);
    let record_id = Uuid::new_v4();

    let saved_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_feedback_logs (
            id, tenant_id, user_id, thread_id, message_id, rating,
            question_text, ai_reply_text, subject_name, grade_level, model_name, reason
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (tenant_id, user_id, message_id)
        DO UPDATE SET
            rating = EXCLUDED.rating,
            question_text = COALESCE(EXCLUDED.question_text, ai_feedback_logs.question_text),
            ai_reply_text = EXCLUDED.ai_reply_text,
            reason = EXCLUDED.reason,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(record_id)
    .bind(req_ctx.tenant_id)
    .bind(user_id)
    .bind(thread_id)
    .bind(msg_id)
    .bind(normalized_rating)
    .bind(payload.question_text.as_deref())
    .bind(payload.ai_reply_text.trim())
    .bind(payload.subject_name.as_deref())
    .bind(payload.grade_level.as_deref())
    .bind(payload.model_name.as_deref())
    .bind(payload.reason.as_deref())
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Gagal menyimpan feedback AI: {e}")),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(
        AiFeedbackResponse {
            id: saved_id,
            rating: normalized_rating.to_string(),
            recorded: true,
        },
        req_ctx.request_id,
    )))
}
