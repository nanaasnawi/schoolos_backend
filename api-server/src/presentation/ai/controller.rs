use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    infrastructure::nvidia_ai::{
        self, GeneratedArticleDto, GeneratedAssignmentDto, GeneratedCbtQuizDto,
        GeneratedInfographicDto, StudentAnalyticsInsightDto,
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
pub struct AnalyzeStudentRequest {
    pub student_id: Uuid,
}

pub fn ai_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/generate-content", post(generate_content))
        .route("/analytics/student", post(analyze_student))
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
    State(_ctx): State<ApplicationContext>,
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

    match payload.mode.to_uppercase().as_str() {
        "INFOGRAPHIC" => {
            let info = nvidia_ai::generate_infographic(topic, grade, subject)
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
            let is_homework = format.eq_ignore_ascii_case("HOMEWORK_PR")
                || format.to_uppercase().contains("HOMEWORK")
                || format.to_uppercase().contains("PR");

            let num_mcq = if is_homework {
                0
            } else {
                payload.num_mcq
                    .or_else(|| payload.num_questions.map(|q| (q * 7 / 10).max(1)))
                    .unwrap_or(4)
                    .clamp(0, 25)
            };

            let num_essay = if is_homework {
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

            let include_images = payload.include_images.unwrap_or(false);

            let task = nvidia_ai::generate_assignment(
                topic,
                grade,
                subject,
                Some(format),
                num_mcq,
                num_essay,
                diff,
                include_images,
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
            let is_mixed = quiz_format
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
                .clamp(0, 30);
            let num_essay = if is_mixed {
                payload.num_essay
                    .or_else(|| payload.num_questions.map(|q| q.saturating_sub(num_mcq).max(1)))
                    .unwrap_or(5)
                    .clamp(0, 20)
            } else {
                0
            };
            let include_images = payload.include_images.unwrap_or(false);

            let quiz = nvidia_ai::generate_cbt_quiz(
                topic,
                grade,
                subject,
                quiz_format,
                num_mcq,
                num_essay,
                diff,
                include_images,
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
