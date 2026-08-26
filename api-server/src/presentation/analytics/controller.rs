use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

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

pub fn analytics_routes() -> Router<ApplicationContext> {
    Router::new().route("/overview", get(get_overview))
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

    let total_students = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let active_students = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND status IN ('Active', 'active') AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_teachers = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_tendik = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_classes = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let active_classes = total_classes; // For now, all classes are active

    let total_guardians = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM guardians WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(req_ctx.tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let response_data = AnalyticsOverviewResponse {
        total_students,
        active_students,
        total_teachers,
        total_tendik,
        total_classes,
        active_classes,
        total_guardians,
        attendance_rate: 96.2, // Mocked for UI phase 3
        at_risk_students: 11, // Mocked for UI phase 3
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.correlation_id,
    )))
}
