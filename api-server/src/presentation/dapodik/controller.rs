use axum::{
    routing::{get, post},
    Router,
};

use crate::bootstrap::ApplicationContext;

pub use super::excel::*;
pub use super::prefill::*;
pub use super::qr::*;
pub use super::sync::*;

pub fn dapodik_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/health-check", get(check_dapodik_health))
        .route("/sync-records", get(list_sync_records))
        .route("/outbox-jobs", get(list_outbox_jobs))
        .route("/pull", post(pull_dapodik_records))
        .route("/agent/sync", post(pull_dapodik_records))
        .route("/agent/info", get(get_agent_info))
        .route("/push", post(push_dapodik_job))
        .route("/prefill/generate", post(generate_prefill_dapodik))
        .route("/prefill/upload", post(upload_prefill_file))
        .route("/import-excel", post(import_excel_dapodik))
        .route("/reconcile", post(reconcile_student))
        .route("/qr/create", post(create_qr_token))
        .route("/qr/claim", post(claim_qr_token))
}
