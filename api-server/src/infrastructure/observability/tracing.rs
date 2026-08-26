use axum::{extract::Request, middleware::Next, response::IntoResponse};
use tracing::{Instrument, info_span};
use uuid::Uuid;

pub async fn tracing_middleware(req: Request, next: Next) -> impl IntoResponse {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let correlation_id = req
        .headers()
        .get("x-correlation-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| request_id.clone());

    // Extract tenant_id from headers (or from your existing context mechanism if you prefer)
    let tenant_id = req
        .headers()
        .get("x-tenant-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("system");

    // In a real app, actor_id comes from auth middleware. We can just use "anonymous" as fallback.
    let actor_id = req
        .extensions()
        .get::<crate::extractors::RequestContext>()
        .and_then(|ctx| ctx.actor.as_ref().map(|a| a.id.to_string()))
        .unwrap_or_else(|| "anonymous".to_string());

    let span = info_span!(
        "http_request",
        request_id = %request_id,
        correlation_id = %correlation_id,
        tenant_id = %tenant_id,
        actor_id = %actor_id,
        method = %req.method(),
        uri = %req.uri(),
    );

    next.run(req).instrument(span).await
}
