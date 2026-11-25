use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::IntoResponse,
};
use metrics::{counter, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

use std::sync::OnceLock;

static RECORDER_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn setup_metrics_recorder() -> PrometheusHandle {
    RECORDER_HANDLE
        .get_or_init(|| {
            PrometheusBuilder::new()
                .install_recorder()
                .expect("Failed to install Prometheus recorder")
        })
        .clone()
}

pub async fn metrics_handler(
    headers: axum::http::HeaderMap,
    handle: axum::extract::State<PrometheusHandle>,
) -> axum::response::Response {
    use axum::http::StatusCode;

    if let Ok(expected_token) = std::env::var("METRICS_TOKEN") {
        let trimmed = expected_token.trim();
        if !trimmed.is_empty() {
            let provided = headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
                .map(str::trim);
            if provided != Some(trimmed) {
                return (StatusCode::UNAUTHORIZED, "Unauthorized metrics access").into_response();
            }
            return handle.render().into_response();
        }
    }

    let is_prod = std::env::var("RAILWAY_ENVIRONMENT")
        .or_else(|_| std::env::var("APP_ENV"))
        .or_else(|_| std::env::var("RUST_ENV"))
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false);
    if is_prod {
        return (
            StatusCode::UNAUTHORIZED,
            "Metrics endpoint disabled in production without METRICS_TOKEN",
        )
            .into_response();
    }

    handle.render().into_response()
}

pub async fn metrics_middleware(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();

    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };

    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("route", path),
        ("status", status),
    ];

    counter!("http_requests_total", &labels).increment(1);
    histogram!("http_request_duration_seconds", &labels).record(latency);

    response
}
