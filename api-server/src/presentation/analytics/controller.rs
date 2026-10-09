use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
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
    pub attendance_rate: f64,  // Mocked for now
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
        .route("/schedule-compliance", get(get_schedule_compliance))
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
            // Setelah migration 20260930, semua status sudah dinormalisasi ke lowercase.
            // 'late' (terlambat) juga dihitung sebagai hadir (masuk sekolah).
            "SELECT COUNT(*) FROM session_attendances WHERE tenant_id = $1 AND status IN ('present', 'late')",
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

    if let Some(ref actor) = req_ctx.actor {
        let is_admin_or_staff = actor.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin") || n.contains("kepala") || n.contains("staff") || n.contains("tata usaha")
        });
        if !is_admin_or_staff {
            return Err(ApiError::new(
                ApplicationError::Forbidden(
                    ErrorCode::AuthPermissionDenied,
                    "Akses ditolak: Portal administrator dan analitik sekolah hanya dapat diakses oleh Administrator & Staf Tata Usaha.".to_string(),
                ),
                req_ctx.request_id,
            ));
        }
    }

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

    let total_students_num = if total_students > 0 {
        total_students
    } else {
        1
    } as f64;

    // Gender distribution
    let gender_rows = sqlx::query(
        r#"
        SELECT gender, COUNT(*)::int8 AS count
        FROM students
        WHERE tenant_id = $1 AND deleted_at IS NULL
        GROUP BY gender
        ORDER BY gender
        "#,
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut male_count = 0i64;
    let mut female_count = 0i64;
    for r in gender_rows {
        let g: Option<String> = r.get("gender");
        let g_str = g.as_deref().unwrap_or("").to_uppercase();
        let cnt: i64 = r.get("count");
        if g_str == "L" || g_str == "MALE" || g_str == "LAKI-LAKI" {
            male_count += cnt;
        } else if g_str == "P" || g_str == "FEMALE" || g_str == "PEREMPUAN" {
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
    let jenjang_rows = sqlx::query(
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
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let jenjang_distribution = jenjang_rows
        .into_iter()
        .filter_map(|r| {
            let j: Option<String> = r.get("jenjang");
            let j_val = j?;
            let s_cnt: i64 = r.get("student_count");
            let c_cnt: i64 = r.get("class_count");
            if j_val != "Lainnya" || s_cnt > 0 {
                Some(JenjangDistributionDto {
                    jenjang: j_val,
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
    let rombel_rows = sqlx::query(
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
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let rombel_distribution = rombel_rows
        .into_iter()
        .map(|r| RombelDistributionDto {
            id: r.get("id"),
            name: r.get("name"),
            jenis_rombel: r.get("jenis_rombel"),
            student_count: r.get("student_count"),
        })
        .collect();

    // Academic performance
    let academic_rows = sqlx::query(
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
        "#
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let academic_performance = academic_rows
        .into_iter()
        .map(|r| AcademicPerformanceDto {
            subject_id: r.get("subject_id"),
            subject_name: r.get("subject_name"),
            subject_code: r.get("subject_code"),
            total_graded: r.get("total_graded"),
            average_score: r.get("average_score"),
            min_score: r.get("min_score"),
            max_score: r.get("max_score"),
            passed_count: r.get("passed_count"),
            remedial_count: r.get("remedial_count"),
        })
        .collect();

    // Announcements
    let announcements_rows = sqlx::query(
        r#"
        SELECT 
            id, title, content, category, target, author, is_pinned, push_status, created_at
        FROM announcements
        WHERE tenant_id = $1
        ORDER BY is_pinned DESC, created_at DESC
        LIMIT 3
        "#,
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let announcements = announcements_rows
        .into_iter()
        .map(|r| DashboardAnnouncementDto {
            id: r.get("id"),
            title: r.get("title"),
            content: r.get("content"),
            category: r.get("category"),
            target: r.get("target"),
            author: r.get("author"),
            is_pinned: r.get("is_pinned"),
            push_status: r.get("push_status"),
            created_at: r.get("created_at"),
        })
        .collect();

    // Audit logs / recent activities
    let audit_rows = sqlx::query(
        r#"
        SELECT 
            id, action, resource, decision, reason, timestamp AS created_at
        FROM audit_logs
        WHERE tenant_id = $1
        ORDER BY timestamp DESC
        LIMIT 6
        "#,
    )
    .bind(tid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_activities = audit_rows
        .into_iter()
        .map(|r| DashboardRecentActivityDto {
            id: r.get("id"),
            action: r.get("action"),
            resource: r.get("resource"),
            decision: r.get("decision"),
            reason: r.get("reason"),
            created_at: r.get("created_at"),
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

#[derive(Debug, Deserialize, Default)]
pub struct ScheduleComplianceFilterQuery {
    pub date: Option<chrono::NaiveDate>,
    pub class_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ComplianceItemDto {
    pub schedule_id: Uuid,
    pub session_id: Option<Uuid>,
    pub class_id: Uuid,
    pub class_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_id: Uuid,
    pub teacher_name: String,
    pub substitute_teacher_id: Option<Uuid>,
    pub substitute_teacher_name: Option<String>,
    pub day_of_week: String,
    pub date: chrono::NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub room: String,
    pub state: String,
    pub attendance_count: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ScheduleComplianceResponse {
    pub date: chrono::NaiveDate,
    pub total_scheduled: i64,
    pub completed_count: i64,
    pub in_progress_count: i64,
    pub scheduled_count: i64,
    pub substituted_count: i64,
    pub cancelled_count: i64,
    pub overdue_unrecorded_count: i64,
    pub compliance_rate: f64,
    pub items: Vec<ComplianceItemDto>,
}

async fn get_schedule_compliance(
    State(ctx): State<ApplicationContext>,
    Query(filter): Query<ScheduleComplianceFilterQuery>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<ScheduleComplianceResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;

    require_permission(&req_ctx.actor, Permission::AcademicManage)
        .or_else(|_| require_permission(&req_ctx.actor, Permission::TeacherRead))
        .map_err(|_| {
            ApiError::new(
                ApplicationError::Unauthorized(
                    ErrorCode::AuthPermissionDenied,
                    "Insufficient permissions to view schedule compliance".to_string(),
                ),
                &req_ctx.request_id,
            )
        })?;

    let now_utc = chrono::Utc::now();
    let today = now_utc.date_naive();
    let target_date = filter.date.unwrap_or(today);

    let day_name = match target_date.weekday() {
        chrono::Weekday::Mon => "senin",
        chrono::Weekday::Tue => "selasa",
        chrono::Weekday::Wed => "rabu",
        chrono::Weekday::Thu => "kamis",
        chrono::Weekday::Fri => "jumat",
        chrono::Weekday::Sat => "sabtu",
        chrono::Weekday::Sun => "minggu",
    };

    let current_time_wib = (now_utc + chrono::Duration::hours(7)).time();

    let rows = sqlx::query(
        r#"
        SELECT 
            cs.id as schedule_id,
            cs.class_id,
            c.name as class_name,
            cs.subject_id,
            s.name as subject_name,
            cs.teacher_id,
            t.full_name as teacher_name,
            cs.day_of_week,
            cs.start_time,
            cs.end_time,
            COALESCE(cs.room, 'Ruang Kelas') as room,
            ls.id as session_id,
            ls.substitute_teacher_id,
            st.full_name as substitute_teacher_name,
            ls.status as session_status,
            ls.started_at,
            ls.ended_at,
            ls.notes,
            COALESCE((
                SELECT COUNT(*)::bigint 
                FROM session_attendances sa 
                WHERE sa.session_id = ls.id AND sa.tenant_id = cs.tenant_id
            ), 0) as attendance_count
        FROM class_schedules cs
        JOIN classes c ON c.id = cs.class_id
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        LEFT JOIN learning_sessions ls ON ls.schedule_id = cs.id 
                                      AND ls.session_date = $2 
                                      AND ls.tenant_id = cs.tenant_id 
                                      AND ls.deleted_at IS NULL
        LEFT JOIN teachers st ON st.id = ls.substitute_teacher_id
        WHERE cs.tenant_id = $1
          AND LOWER(TRIM(cs.day_of_week)) = $3
          AND cs.deleted_at IS NULL
          AND ($4::uuid IS NULL OR cs.class_id = $4)
          AND ($5::uuid IS NULL OR cs.teacher_id = $5)
        ORDER BY cs.start_time ASC
        "#
    )
    .bind(req_ctx.tenant_id)
    .bind(target_date)
    .bind(day_name)
    .bind(filter.class_id)
    .bind(filter.teacher_id)
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

    let mut items = Vec::new();
    let mut completed_count = 0i64;
    let mut in_progress_count = 0i64;
    let mut scheduled_count = 0i64;
    let mut substituted_count = 0i64;
    let mut cancelled_count = 0i64;
    let mut overdue_unrecorded_count = 0i64;

    for r in rows {
        let schedule_id: Uuid = r.get("schedule_id");
        let class_id: Uuid = r.get("class_id");
        let class_name: String = r.get("class_name");
        let subject_id: Uuid = r.get("subject_id");
        let subject_name: String = r.get("subject_name");
        let teacher_id: Uuid = r.get("teacher_id");
        let teacher_name: String = r.get("teacher_name");
        let day_of_week: String = r.get("day_of_week");
        let start_time: String = r.get("start_time");
        let end_time: String = r.get("end_time");
        let room: String = r.get("room");
        let session_id: Option<Uuid> = r.get("session_id");
        let substitute_teacher_id: Option<Uuid> = r.get("substitute_teacher_id");
        let substitute_teacher_name: Option<String> = r.get("substitute_teacher_name");
        let session_status: Option<String> = r.get("session_status");
        let ended_at: Option<chrono::DateTime<chrono::Utc>> = r.get("ended_at");
        let started_at: Option<chrono::DateTime<chrono::Utc>> = r.get("started_at");
        let notes: Option<String> = r.get("notes");
        let attendance_count: i64 = r.get("attendance_count");

        let state = if let Some(ref st) = session_status {
            if st == "cancelled" {
                cancelled_count += 1;
                "CANCELLED"
            } else if substitute_teacher_id.is_some() {
                substituted_count += 1;
                "SUBSTITUTED"
            } else if st == "completed" || ended_at.is_some() {
                completed_count += 1;
                "COMPLETED"
            } else if st == "active" || started_at.is_some() {
                in_progress_count += 1;
                "IN_PROGRESS"
            } else {
                let parsed_end = chrono::NaiveTime::parse_from_str(end_time.trim(), "%H:%M:%S")
                    .or_else(|_| chrono::NaiveTime::parse_from_str(end_time.trim(), "%H:%M"))
                    .ok();

                let is_overdue = if target_date < today {
                    true
                } else if target_date == today {
                    parsed_end.map(|et| current_time_wib > et).unwrap_or(false)
                } else {
                    false
                };

                if is_overdue {
                    overdue_unrecorded_count += 1;
                    "OVERDUE_UNRECORDED"
                } else {
                    scheduled_count += 1;
                    "SCHEDULED"
                }
            }
        } else {
            let parsed_end = chrono::NaiveTime::parse_from_str(end_time.trim(), "%H:%M:%S")
                .or_else(|_| chrono::NaiveTime::parse_from_str(end_time.trim(), "%H:%M"))
                .ok();

            let is_overdue = if target_date < today {
                true
            } else if target_date == today {
                parsed_end.map(|et| current_time_wib > et).unwrap_or(false)
            } else {
                false
            };

            if is_overdue {
                overdue_unrecorded_count += 1;
                "OVERDUE_UNRECORDED"
            } else {
                scheduled_count += 1;
                "SCHEDULED"
            }
        };

        items.push(ComplianceItemDto {
            schedule_id,
            session_id,
            class_id,
            class_name,
            subject_id,
            subject_name,
            teacher_id,
            teacher_name,
            substitute_teacher_id,
            substitute_teacher_name,
            day_of_week,
            date: target_date,
            start_time,
            end_time,
            room,
            state: state.to_string(),
            attendance_count,
            notes,
        });
    }

    let total_scheduled = items.len() as i64;
    let effective_denominator = total_scheduled - cancelled_count;
    let compliance_rate = if effective_denominator > 0 {
        ((completed_count + substituted_count) as f64 / effective_denominator as f64) * 100.0
    } else {
        100.0
    };

    Ok(Json(ApiResponse::success(
        ScheduleComplianceResponse {
            date: target_date,
            total_scheduled,
            completed_count,
            in_progress_count,
            scheduled_count,
            substituted_count,
            cancelled_count,
            overdue_unrecorded_count,
            compliance_rate,
            items,
        },
        req_ctx.request_id,
    )))
}
