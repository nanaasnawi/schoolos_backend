use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::IntoResponse,
};
use metrics::{counter, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

pub fn setup_metrics_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
}

pub async fn metrics_handler(handle: axum::extract::State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
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
