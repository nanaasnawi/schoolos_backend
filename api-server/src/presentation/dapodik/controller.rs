use axum::{
    routing::{get, post},
    Router,
};
use school_core::common::{error::ApplicationError, error_code::ErrorCode};

use crate::{bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext};

pub use super::excel::*;
pub use super::prefill::*;
pub use super::qr::*;
pub use super::sync::*;

pub fn is_dapodik_admin_actor(ctx: &RequestContext) -> bool {
    ctx.actor
        .as_ref()
        .map(|actor| {
            actor.roles.iter().any(|r| {
                let lower = r.name.trim().to_lowercase();
                lower.contains("admin")
                    || lower.contains("operator")
                    || lower.contains("kepala")
                    || lower.contains("staff")
                    || lower.contains("tata usaha")
                    || lower == "tu"
            })
        })
        .unwrap_or(false)
}

pub fn require_dapodik_admin(ctx: &RequestContext) -> Result<(), ApiError> {
    if !is_dapodik_admin_actor(ctx) {
        return Err(ApiError::new(
            ApplicationError::Forbidden(
                ErrorCode::AuthPermissionDenied,
                "Hanya Administrator, Operator Dapodik, atau Kepala Sekolah yang diizinkan mengakses modul Dapodik".to_string(),
            ),
            &ctx.request_id,
        ));
    }
    Ok(())
}

pub fn dapodik_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/health-check", get(check_dapodik_health))
        .route("/sync-records", get(list_sync_records))
        .route("/outbox-jobs", get(list_outbox_jobs))
        .route("/pull", post(pull_dapodik_records))
        .route("/agent/sync", post(agent_sync_async_handler))
        .route("/agent/info", get(get_agent_info))
        .route("/sync-status", get(get_agent_info))
        .route("/push", post(push_dapodik_job))
        .route("/prefill/generate", post(generate_prefill_dapodik))
        .route("/prefill/upload", post(upload_prefill_file))
        .route("/import-excel", post(import_excel_dapodik))
        .route("/reconcile", post(reconcile_student))
        .route("/qr/create", post(create_qr_token))
        .route("/qr/claim", post(claim_qr_token))
}
