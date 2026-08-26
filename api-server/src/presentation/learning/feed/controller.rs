use axum::{
    Json, Router,
    extract::{Query, State},
    routing::post,
};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    create_feed_item_request::CreateFeedItemRequest, feed_item_response::FeedItemResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::models::page::Page;
use school_core::learning::application::feed::{
    create_feed_item::CreateFeedItemCommand, list_feed::ListFeedQuery,
};

#[derive(Debug, Deserialize)]
pub struct FeedQueryParams {
    pub class_id: Uuid,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub fn feed_routes() -> Router<ApplicationContext> {
    Router::new().route("/", post(create).get(list))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateFeedItemRequest>,
) -> Result<Json<ApiResponse<FeedItemResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningFeedCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateFeedItemCommand {
        tenant_id: req_ctx.tenant_id,
        class_id: payload.class_id,
        actor_id: payload.actor_id,
        actor_name: payload.actor_name,
        action: payload.action,
        target_type: payload.target_type,
        target_id: payload.target_id,
        summary: payload.summary,
        metadata: payload.metadata.unwrap_or_default(),
    };

    let item = ctx
        .create_feed_item
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        FeedItemResponse::from(item),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<FeedQueryParams>,
) -> Result<Json<ApiResponse<Page<FeedItemResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningFeedRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListFeedQuery {
        class_id: params.class_id,
        tenant_id: req_ctx.tenant_id,
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(20),
    };

    let page = ctx
        .list_feed
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = page.items.into_iter().map(FeedItemResponse::from).collect();
    let response_page = Page::new(items, page.total_items as i64, page.page, page.page_size);

    Ok(Json(ApiResponse::success(
        response_page,
        req_ctx.request_id,
    )))
}
