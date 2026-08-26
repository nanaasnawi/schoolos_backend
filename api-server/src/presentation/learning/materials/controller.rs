use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    create_learning_material_request::CreateLearningMaterialRequest,
    learning_material_response::LearningMaterialResponse,
    update_learning_material_request::UpdateLearningMaterialRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::learning_material::{
    create_learning_material::CreateLearningMaterialCommand,
    delete_learning_material::DeleteLearningMaterialCommand,
    get_learning_material::GetLearningMaterialQuery,
    list_learning_materials::ListLearningMaterialsQuery,
    update_learning_material::UpdateLearningMaterialCommand,
};

pub fn material_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        material_type: payload.material_type,
        title: payload.title,
        description: payload.description,
        storage_key: payload.storage_key,
        external_url: payload.external_url,
        order_index: payload.order_index,
        visibility: payload.visibility,
    };

    let material = ctx
        .create_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LearningMaterialResponse::from(material),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<LearningMaterialResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListLearningMaterialsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let materials = ctx
        .list_learning_materials
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = materials
        .into_iter()
        .map(LearningMaterialResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetLearningMaterialQuery {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
    };

    let material = ctx
        .get_learning_material
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LearningMaterialResponse::from(material),
        req_ctx.request_id,
    )))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateLearningMaterialRequest>,
) -> Result<Json<ApiResponse<LearningMaterialResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
        title: payload.title,
        description: payload.description,
        storage_key: payload.storage_key,
        external_url: payload.external_url,
        visibility: payload.visibility,
    };

    let material = ctx
        .update_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        LearningMaterialResponse::from(material),
        req_ctx.request_id,
    )))
}

async fn delete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningMaterialDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let command = DeleteLearningMaterialCommand {
        tenant_id: req_ctx.tenant_id,
        material_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_learning_material
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}
