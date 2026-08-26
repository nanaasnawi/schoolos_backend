use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
};
use serde::Deserialize;
use uuid::Uuid;

use super::dto::{
    notification_response::{NotificationResponse, UnreadCountResponse},
    preference_response::PreferenceResponse,
    upsert_preference_request::UpsertPreferenceRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::models::page::Page;
use school_core::notification::application::{
    get_preferences::GetPreferencesQuery, list_notifications::ListNotificationsQuery,
    mark_all_read::MarkAllNotificationsReadCommand, mark_read::MarkNotificationReadCommand,
    upsert_preference::UpsertPreferenceCommand,
};
use school_core::notification::domain::notification_channel::NotificationChannel;

#[derive(Debug, Deserialize)]
pub struct NotificationQueryParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub fn notification_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", get(list))
        .route("/unread-count", get(unread_count))
        .route("/{id}/read", patch(mark_read))
        .route("/read-all", patch(mark_all_read))
        .route("/preferences", get(get_preferences).post(upsert_preference))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<NotificationQueryParams>,
) -> Result<Json<ApiResponse<Page<NotificationResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let query = ListNotificationsQuery {
        user_id,
        tenant_id: req_ctx.tenant_id,
        page: params.page.unwrap_or(1),
        per_page: params.per_page.unwrap_or(20),
    };

    let page = ctx
        .list_notifications
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = page
        .items
        .into_iter()
        .map(NotificationResponse::from)
        .collect();
    let response_page = Page::new(items, page.total_items as i64, page.page, page.page_size);

    Ok(Json(ApiResponse::success(
        response_page,
        req_ctx.request_id,
    )))
}

async fn unread_count(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<UnreadCountResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let count = ctx
        .notification_repo
        .count_unread(user_id, req_ctx.tenant_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(e),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(
        UnreadCountResponse { count },
        req_ctx.request_id,
    )))
}

async fn mark_read(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = MarkNotificationReadCommand {
        notification_id: id,
        user_id,
    };

    ctx.mark_notification_read
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn mark_all_read(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = MarkAllNotificationsReadCommand {
        user_id,
        tenant_id: req_ctx.tenant_id,
    };

    ctx.mark_all_notifications_read
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn get_preferences(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<PreferenceResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let query = GetPreferencesQuery {
        user_id,
        tenant_id: req_ctx.tenant_id,
    };

    let prefs = ctx
        .get_notification_preferences
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = prefs.into_iter().map(PreferenceResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn upsert_preference(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<UpsertPreferenceRequest>,
) -> Result<Json<ApiResponse<PreferenceResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::NotificationUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let channels: Vec<NotificationChannel> = payload
        .channels
        .iter()
        .filter_map(|c| NotificationChannel::from_str(c))
        .collect();

    let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = UpsertPreferenceCommand {
        tenant_id: req_ctx.tenant_id,
        user_id,
        notification_type: payload.notification_type,
        channels,
        is_enabled: payload.is_enabled,
    };

    let pref = ctx
        .upsert_notification_preference
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        PreferenceResponse::from(pref),
        req_ctx.request_id,
    )))
}
