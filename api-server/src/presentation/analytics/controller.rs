use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalyticsOverviewResponse {
    pub total_students: i64,
    pub active_students: i64,
    pub total_teachers: i64,
    pub total_tendik: i64,
    pub total_classes: i64,
    pub active_classes: i64,
    pub total_guardians: i64,
    pub attendance_rate: f64, // Mocked for now
    pub at_risk_students: i64, // Mocked for now
}

#[derive(Serialize, Deserialize, utoipa::ToSchema, Default)]
pub struct DashboardMetricsDto {
    pub total_students: i64,
    pub active_students: i64,
    pub transferred_students: i64,
    pub total_teachers: i64,
    pub active_teachers: i64,
    pub total_tendik: i64,
    pub total_classes: i64,
    pub active_classes: i64,
    pub total_guardians: i64,
    pub active_qr_tokens: i64,
    pub total_learning_materials: i64,
    pub total_quizzes: i64,
    pub total_assignments: i64,
    pub total_submissions: i64,
    pub dapodik_sync_records: i64,
    pub total_notifications: i64,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct GenderDistributionDto {
    pub gender: String,
    pub label: String,
    pub count: i64,
    pub percentage: f64,
    pub color: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct JenjangDistributionDto {
    pub jenjang: String,
    pub student_count: i64,
    pub class_count: i64,
    pub percentage: f64,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct RombelDistributionDto {
    pub id: Uuid,
    pub name: String,
    pub jenis_rombel: Option<String>,
    pub student_count: i64,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct AcademicPerformanceDto {
    pub subject_id: Uuid,
    pub subject_name: String,
    pub subject_code: String,
    pub total_graded: i64,
    pub average_score: f64,
    pub min_score: f64,
    pub max_score: f64,
    pub passed_count: i64,
    pub remedial_count: i64,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct DashboardAnnouncementDto {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub category: String,
    pub target: String,
    pub author: String,
    pub is_pinned: bool,
    pub push_status: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct DashboardRecentActivityDto {
    pub id: Uuid,
    pub action: String,
    pub resource: Option<String>,
    pub decision: String,
    pub reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct DashboardDataResponse {
    pub metrics: DashboardMetricsDto,
    pub gender_distribution: Vec<GenderDistributionDto>,
    pub jenjang_distribution: Vec<JenjangDistributionDto>,
    pub rombel_distribution: Vec<RombelDistributionDto>,
    pub academic_performance: Vec<AcademicPerformanceDto>,
    pub announcements: Vec<DashboardAnnouncementDto>,
    pub recent_activities: Vec<DashboardRecentActivityDto>,
}

pub fn analytics_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/overview", get(get_overview))
        .route("/dashboard", get(get_dashboard))
}

#[utoipa::path(
    get,
    operation_id = "getAnalyticsOverview",
    path = "/api/v1/analytics/overview",
    responses(
        (status = 200, description = "Analytics Overview", body = ApiResponse<AnalyticsOverviewResponse>),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Analytics"
)]
async fn get_overview(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<AnalyticsOverviewResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::SchoolUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let pool = &ctx.pool;

    use tokio::join;

    let (total_students_res, active_students_res, total_teachers_res, total_tendik_res, total_classes_res, total_guardians_res, at_risk_students_res, total_attendances_res, present_attendances_res) = join!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND status IN ('Active', 'active') AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM staff WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM guardians WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT student_id) FROM gradebooks WHERE tenant_id = $1 AND passed = false",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_attendances WHERE tenant_id = $1",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_attendances WHERE tenant_id = $1 AND status IN ('Present', 'present', 'Hadir', 'hadir')",
        )
        .bind(req_ctx.tenant_id)
        .fetch_one(pool),
    );

    let total_students = total_students_res.unwrap_or(0);
    let active_students = active_students_res.unwrap_or(0);
    let total_teachers = total_teachers_res.unwrap_or(0);
    let total_tendik = total_tendik_res.unwrap_or(0);
    let total_classes = total_classes_res.unwrap_or(0);
    let active_classes = total_classes; // For now, all classes are active
    let total_guardians = total_guardians_res.unwrap_or(0);
    let at_risk_students = at_risk_students_res.unwrap_or(0);
    let total_attendances = total_attendances_res.unwrap_or(0);
    let present_attendances = present_attendances_res.unwrap_or(0);

    let attendance_rate = if total_attendances > 0 {
        ((present_attendances as f64 / total_attendances as f64) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let response_data = AnalyticsOverviewResponse {
        total_students,
        active_students,
        total_teachers,
        total_tendik,
        total_classes,
        active_classes,
        total_guardians,
        attendance_rate,
        at_risk_students,
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.correlation_id,
    )))
}

#[utoipa::path(
    get,
    operation_id = "getAnalyticsDashboard",
    path = "/api/v1/analytics/dashboard",
    responses(
        (status = 200, description = "Analytics Dashboard Comprehensive", body = ApiResponse<DashboardDataResponse>),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Analytics"
)]
async fn get_dashboard(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<DashboardDataResponse>>, ApiError> {
    let pool = &ctx.pool;
    let tid = req_ctx.tenant_id;

    let (
        total_students_res,
        active_students_res,
        transferred_students_res,
        total_teachers_res,
        active_teachers_res,
        total_tendik_res,
        total_classes_res,
        active_classes_res,
        total_guardians_res,
        active_qr_tokens_res,
        total_materials_res,
        total_quizzes_res,
        total_assignments_res,
        total_submissions_res,
        dapodik_records_res,
        total_notifications_res,
    ) = tokio::join!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND status IN ('Active', 'active') AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND status IN ('transferred', 'Transferred') AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM staff WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT class_id) FROM enrollments WHERE tenant_id = $1 AND status = 'Active'").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM guardians WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_qr_tokens WHERE tenant_id = $1 AND is_active = true").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM learning_materials WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quizzes WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assignments WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assignment_submissions WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dapodik_sync_records WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notifications WHERE tenant_id = $1").bind(tid).fetch_one(pool),
    );

    let total_students = total_students_res.unwrap_or(0);
    let metrics = DashboardMetricsDto {
        total_students,
        active_students: active_students_res.unwrap_or(0),
        transferred_students: transferred_students_res.unwrap_or(0),
        total_teachers: total_teachers_res.unwrap_or(0),
        active_teachers: active_teachers_res.unwrap_or(0),
        total_tendik: total_tendik_res.unwrap_or(0),
        total_classes: total_classes_res.unwrap_or(0),
        active_classes: active_classes_res.unwrap_or(0),
        total_guardians: total_guardians_res.unwrap_or(0),
        active_qr_tokens: active_qr_tokens_res.unwrap_or(0),
        total_learning_materials: total_materials_res.unwrap_or(0),
        total_quizzes: total_quizzes_res.unwrap_or(0),
        total_assignments: total_assignments_res.unwrap_or(0),
        total_submissions: total_submissions_res.unwrap_or(0),
        dapodik_sync_records: dapodik_records_res.unwrap_or(0),
        total_notifications: total_notifications_res.unwrap_or(0),
    };

    let total_students_num = if total_students > 0 { total_students } else { 1 } as f64;

    // Gender distribution
    let gender_rows = sqlx::query!(
        r#"
        SELECT gender, COUNT(*)::int8 AS count
        FROM students
        WHERE tenant_id = $1 AND deleted_at IS NULL
        GROUP BY gender
        ORDER BY gender
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut male_count = 0i64;
    let mut female_count = 0i64;
    for r in gender_rows {
        let g = r.gender.as_deref().unwrap_or("").to_uppercase();
        let cnt = r.count.unwrap_or(0);
        if g == "L" || g == "MALE" || g == "LAKI-LAKI" {
            male_count += cnt;
        } else if g == "P" || g == "FEMALE" || g == "PEREMPUAN" {
            female_count += cnt;
        }
    }

    let gender_distribution = vec![
        GenderDistributionDto {
            gender: "L".to_string(),
            label: "Laki-laki".to_string(),
            count: male_count,
            percentage: ((male_count as f64 / total_students_num) * 100.0 * 10.0).round() / 10.0,
            color: "#2563eb".to_string(),
        },
        GenderDistributionDto {
            gender: "P".to_string(),
            label: "Perempuan".to_string(),
            count: female_count,
            percentage: ((female_count as f64 / total_students_num) * 100.0 * 10.0).round() / 10.0,
            color: "#ec4899".to_string(),
        },
    ];

    // Jenjang distribution
    let jenjang_rows = sqlx::query!(
        r#"
        SELECT 
            CASE 
                WHEN c.name LIKE 'PAKET A%' THEN 'Paket A (Setara SD)'
                WHEN c.name LIKE 'PAKET B%' THEN 'Paket B (Setara SMP)'
                WHEN c.name LIKE 'PAKET C%' THEN 'Paket C (Setara SMA)'
                ELSE 'Lainnya'
            END AS jenjang,
            COUNT(e.id)::int8 AS student_count,
            COUNT(DISTINCT c.id)::int8 AS class_count
        FROM classes c
        LEFT JOIN enrollments e ON e.class_id = c.id AND e.status = 'Active'
        WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
        GROUP BY 1
        ORDER BY 1
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let jenjang_distribution = jenjang_rows
        .into_iter()
        .filter_map(|r| {
            let j = r.jenjang?;
            let s_cnt = r.student_count.unwrap_or(0);
            let c_cnt = r.class_count.unwrap_or(0);
            if j != "Lainnya" || s_cnt > 0 {
                Some(JenjangDistributionDto {
                    jenjang: j,
                    student_count: s_cnt,
                    class_count: c_cnt,
                    percentage: ((s_cnt as f64 / total_students_num) * 100.0 * 10.0).round() / 10.0,
                })
            } else {
                None
            }
        })
        .collect();

    // Rombel distribution
    let rombel_rows = sqlx::query!(
        r#"
        SELECT 
            c.id, 
            c.name, 
            c.jenis_rombel,
            COUNT(e.id)::int8 AS student_count
        FROM classes c
        LEFT JOIN enrollments e ON e.class_id = c.id AND e.status = 'Active'
        WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
        GROUP BY c.id, c.name, c.jenis_rombel
        HAVING COUNT(e.id) > 0
        ORDER BY student_count DESC, c.name ASC
        LIMIT 15
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let rombel_distribution = rombel_rows
        .into_iter()
        .map(|r| RombelDistributionDto {
            id: r.id,
            name: r.name,
            jenis_rombel: r.jenis_rombel,
            student_count: r.student_count.unwrap_or(0),
        })
        .collect();

    // Academic performance
    let academic_rows = sqlx::query!(
        r#"
        SELECT 
            s.id AS subject_id,
            s.name AS subject_name,
            s.code AS subject_code,
            COUNT(g.id)::int8 AS total_graded,
            ROUND(AVG(CASE WHEN g.final_score > 0 THEN g.final_score ELSE NULL END), 1)::float8 AS average_score,
            COALESCE(MIN(g.final_score)::float8, 0.0) AS min_score,
            COALESCE(MAX(g.final_score)::float8, 0.0) AS max_score,
            COUNT(CASE WHEN g.passed = true THEN 1 END)::int8 AS passed_count,
            COUNT(CASE WHEN g.passed = false THEN 1 END)::int8 AS remedial_count
        FROM gradebooks g
        JOIN subjects s ON s.id = g.subject_id
        WHERE g.tenant_id = $1
        GROUP BY s.id, s.name, s.code
        HAVING MAX(g.final_score) > 0
        ORDER BY average_score DESC
        LIMIT 10
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let academic_performance = academic_rows
        .into_iter()
        .map(|r| AcademicPerformanceDto {
            subject_id: r.subject_id,
            subject_name: r.subject_name,
            subject_code: r.subject_code,
            total_graded: r.total_graded.unwrap_or(0),
            average_score: r.average_score.unwrap_or(0.0),
            min_score: r.min_score.unwrap_or(0.0),
            max_score: r.max_score.unwrap_or(0.0),
            passed_count: r.passed_count.unwrap_or(0),
            remedial_count: r.remedial_count.unwrap_or(0),
        })
        .collect();

    // Announcements
    let announcements_rows = sqlx::query!(
        r#"
        SELECT 
            id, title, content, category, target, author, is_pinned, push_status, created_at
        FROM announcements
        WHERE tenant_id = $1
        ORDER BY is_pinned DESC, created_at DESC
        LIMIT 6
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let announcements = announcements_rows
        .into_iter()
        .map(|r| DashboardAnnouncementDto {
            id: r.id,
            title: r.title,
            content: r.content,
            category: r.category,
            target: r.target,
            author: r.author,
            is_pinned: r.is_pinned,
            push_status: r.push_status,
            created_at: r.created_at,
        })
        .collect();

    // Audit logs / recent activities
    let audit_rows = sqlx::query!(
        r#"
        SELECT 
            id, action, resource, decision, reason, timestamp AS created_at
        FROM audit_logs
        WHERE tenant_id = $1
        ORDER BY timestamp DESC
        LIMIT 6
        "#,
        tid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_activities = audit_rows
        .into_iter()
        .map(|r| DashboardRecentActivityDto {
            id: r.id,
            action: r.action,
            resource: r.resource,
            decision: r.decision,
            reason: r.reason,
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(ApiResponse::success(
        DashboardDataResponse {
            metrics,
            gender_distribution,
            jenjang_distribution,
            rombel_distribution,
            academic_performance,
            announcements,
            recent_activities,
        },
        req_ctx.correlation_id,
    )))
}
