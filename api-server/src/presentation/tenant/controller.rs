use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};

use super::dto::{
    provision_tenant_request::ProvisionTenantRequest,
    provision_tenant_response::ProvisionTenantResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;
use school_core::identity::application::tenant::provision_tenant::ProvisionTenantCommand;

pub fn tenant_routes() -> Router<ApplicationContext> {
    Router::new().route("/provision", post(provision))
}

/// Provision a new Tenant and its associated School.
///
/// Requires an `Idempotency-Key` header to prevent duplicate provisioning.
#[utoipa::path(
    post,
    operation_id = "provisionTenant",
    path = "/api/v1/tenants/provision",
    request_body = ProvisionTenantRequest,
    responses(
        (status = 200, description = "Provisioning successful", body = inline(ApiResponse<ProvisionTenantResponse>)),
        (status = 400, description = "Idempotency key missing or invalid")
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Tenant"
)]
async fn provision(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<ProvisionTenantRequest>,
) -> Result<Json<ApiResponse<ProvisionTenantResponse>>, ApiError> {
    if req_ctx.idempotency_key.is_none() {
        return Err(ApiError::new(
            ApplicationError::Domain(school_core::common::error::DomainError::Validation(
                "Missing x-idempotency-key header".to_string(),
            )),
            &req_ctx.request_id,
        ));
    }

    let command = ProvisionTenantCommand {
        tenant_name: payload.tenant_name,
        school_name: payload.school_name,
    };

    let tenant_id = ctx
        .provision_tenant
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = ProvisionTenantResponse {
        tenant_id,
        message: "Provisioning started successfully".to_string(),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}
