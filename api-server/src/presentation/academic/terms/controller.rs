use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;
use crate::error::ApiError;
use crate::extractors::RequestContext;
use crate::response::ApiResponse;

use super::dto::{create_term_request::CreateTermRequest, term_response::TermResponse};

use school_core::academic::application::term::{
    create_term::CreateTermCommand, get_term::GetTermQuery, list_terms::ListTermsQuery,
};

#[derive(Deserialize)]
pub struct ListTermsParams {
    pub academic_year_id: Uuid,
}

pub fn term_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateTermRequest>,
) -> Result<Json<ApiResponse<TermResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateTermCommand {
        academic_year_id: payload.academic_year_id,
        name: payload.name,
    };

    let term = ctx
        .create_term
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        TermResponse::from(term),
        req_ctx.correlation_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ListTermsParams>,
) -> Result<Json<ApiResponse<Vec<TermResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListTermsQuery {
        academic_year_id: params.academic_year_id,
    };

    let terms = ctx
        .list_terms
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items: Vec<TermResponse> = terms.into_iter().map(TermResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.correlation_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<TermResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::AcademicManage).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetTermQuery { id };

    let term = ctx
        .get_term
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        TermResponse::from(term),
        req_ctx.correlation_id,
    )))
}
