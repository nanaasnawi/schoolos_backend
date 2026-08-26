use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    assessment_rule_response::AssessmentRuleResponse,
    calculate_grade_request::CalculateGradeRequest, configure_rules_request::ConfigureRulesRequest,
    gradebook_response::GradeBookResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::assessment::{
    calculate_grade::{CalculateGradeCommand, ComponentScoreInput},
    configure_rules::{ComponentInput, ConfigureRulesCommand},
    get_gradebook::GetGradebookQuery,
    get_rules::GetRulesQuery,
};

pub fn assessment_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/rules", post(configure_rules).get(get_rules))
        .route("/calculate", post(calculate_grade))
        .route("/gradebook", get(get_gradebook))
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

async fn get_gradebook(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<GradebookParams>,
) -> Result<Json<ApiResponse<Vec<GradeBookResponse>>>, ApiError> {
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

    let query = GetGradebookQuery {
        student_id: params.student_id,
        class_id: params.class_id,
        subject_id: params.subject_id,
    };

    let gradebooks = ctx
        .get_gradebook
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = gradebooks
        .into_iter()
        .map(GradeBookResponse::from)
        .collect();
    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

#[derive(serde::Deserialize)]
pub struct GradebookParams {
    pub student_id: Option<Uuid>,
    pub class_id: Uuid,
    pub subject_id: Uuid,
}
