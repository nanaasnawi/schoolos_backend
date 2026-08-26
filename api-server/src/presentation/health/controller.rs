use axum::{Router, extract::State, http::StatusCode, routing::get};

use crate::bootstrap::ApplicationContext;

pub fn health_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/live", get(live_handler))
        .route("/ready", get(ready_handler))
        .route("/startup", get(startup_handler))
}

async fn live_handler() -> (StatusCode, &'static str) {
    (StatusCode::OK, "OK")
}

async fn ready_handler(
    State(ctx): State<ApplicationContext>,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
    match sqlx::query("SELECT 1").execute(&ctx.pool).await {
        Ok(_) => Ok((StatusCode::OK, "Ready")),
        Err(_) => Err((StatusCode::SERVICE_UNAVAILABLE, "Database not ready")),
    }
}

async fn startup_handler(
    State(ctx): State<ApplicationContext>,
) -> Result<(StatusCode, &'static str), (StatusCode, &'static str)> {
    match sqlx::query("SELECT 1").execute(&ctx.pool).await {
        Ok(_) => Ok((StatusCode::OK, "Started")),
        Err(_) => Err((StatusCode::SERVICE_UNAVAILABLE, "Waiting for database")),
    }
}
