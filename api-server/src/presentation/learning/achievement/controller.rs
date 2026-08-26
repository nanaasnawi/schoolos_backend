use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    achievement_response::AchievementResponse, award_achievement_request::AwardAchievementRequest,
    create_achievement_request::CreateAchievementRequest,
    student_achievement_response::StudentAchievementResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::achievement::{
    award_achievement::AwardAchievementCommand, create_achievement::CreateAchievementCommand,
    get_achievement::GetAchievementQuery, get_student_achievements::GetStudentAchievementsQuery,
    list_achievements::ListAchievementsQuery,
};

pub fn achievement_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
        .route("/award", post(award))
        .route("/student/{student_id}", get(get_student_achievements))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAchievementRequest>,
) -> Result<Json<ApiResponse<AchievementResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAchievementCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateAchievementCommand {
        tenant_id: req_ctx.tenant_id,
        title: payload.title,
        description: payload.description.unwrap_or_default(),
        icon: payload.icon.unwrap_or_else(|| "🏆".to_string()),
        criteria_type: payload.criteria_type,
        criteria_value: payload.criteria_value,
    };

    let achievement = ctx
        .create_achievement
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AchievementResponse::from(achievement),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AchievementResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAchievementRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListAchievementsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let achievements = ctx
        .list_achievements
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = achievements
        .into_iter()
        .map(AchievementResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AchievementResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAchievementRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetAchievementQuery { achievement_id: id };

    let achievement = ctx
        .get_achievement
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AchievementResponse::from(achievement),
        req_ctx.request_id,
    )))
}

async fn award(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<AwardAchievementRequest>,
) -> Result<Json<ApiResponse<StudentAchievementResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAchievementAward).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = AwardAchievementCommand {
        tenant_id: req_ctx.tenant_id,
        student_id: payload.student_id,
        achievement_id: payload.achievement_id,
    };

    let sa = ctx
        .award_achievement
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        StudentAchievementResponse::from(sa),
        req_ctx.request_id,
    )))
}

async fn get_student_achievements(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<StudentAchievementResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAchievementRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetStudentAchievementsQuery {
        student_id,
        tenant_id: req_ctx.tenant_id,
    };

    let items = ctx
        .get_student_achievements
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let results = items
        .into_iter()
        .map(StudentAchievementResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(results, req_ctx.request_id)))
}
