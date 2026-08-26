use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    add_competency_request::AddCompetencyRequest, competency_response::CompetencyResponse,
    create_syllabus_request::CreateSyllabusRequest, syllabus_response::SyllabusResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::syllabus::{
    add_competency::AddCompetencyCommand, create_syllabus::CreateSyllabusCommand,
    get_syllabus::GetSyllabusQuery, list_competencies::ListCompetenciesQuery,
    list_syllabuses::ListSyllabusesQuery,
};

pub fn syllabus_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
        .route(
            "/{id}/competencies",
            post(add_competency).get(list_competencies),
        )
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateSyllabusRequest>,
) -> Result<Json<ApiResponse<SyllabusResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSyllabusCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateSyllabusCommand {
        tenant_id: req_ctx.tenant_id,
        curriculum_id: payload.curriculum_id,
        subject_id: payload.subject_id,
        grade_level_id: payload.grade_level_id,
        code: payload.code,
        name: payload.name,
        description: payload.description,
    };

    let syllabus = ctx
        .create_syllabus
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SyllabusResponse::from(syllabus),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SyllabusResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSyllabusRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListSyllabusesQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let syllabuses = ctx
        .list_syllabuses
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = syllabuses.into_iter().map(SyllabusResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SyllabusResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSyllabusRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetSyllabusQuery {
        tenant_id: req_ctx.tenant_id,
        syllabus_id: id,
    };

    let syllabus = ctx
        .get_syllabus
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SyllabusResponse::from(syllabus),
        req_ctx.request_id,
    )))
}

async fn add_competency(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddCompetencyRequest>,
) -> Result<Json<ApiResponse<CompetencyResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSyllabusUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = AddCompetencyCommand {
        tenant_id: req_ctx.tenant_id,
        syllabus_id: id,
        code: payload.code,
        competency_type: payload.competency_type,
        description: payload.description,
        order_index: payload.order_index,
    };

    let competency = ctx
        .add_competency
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        CompetencyResponse::from(competency),
        req_ctx.request_id,
    )))
}

async fn list_competencies(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<CompetencyResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSyllabusRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListCompetenciesQuery { syllabus_id: id };

    let competencies = ctx
        .list_competencies
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = competencies
        .into_iter()
        .map(CompetencyResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}
