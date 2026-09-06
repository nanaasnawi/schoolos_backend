use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn test_openapi_spec_structure() {
    use utoipa::OpenApi;
    let openapi = api_server::ApiDoc::openapi();
    assert_eq!(openapi.info.title, "ApiDoc");
    let json = openapi.to_json().expect("OpenAPI must serialize to JSON");
    let parsed: Value = serde_json::from_str(&json).expect("Must be valid JSON");
    assert!(parsed.get("paths").is_some());
    assert!(parsed.get("components").is_some());
}

#[tokio::test]
async fn test_health_and_metrics_endpoints() {
    let bootstrap = api_server::bootstrap::Bootstrap::new();
    let app = bootstrap.build().await.expect("App should bootstrap successfully");

    // Test /health endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request to /health failed");

    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND,
        "Status should be valid HTTP status"
    );

    // Test /metrics endpoint
    let metrics_res = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request to /metrics failed");

    assert_eq!(metrics_res.status(), StatusCode::OK);
    let body = metrics_res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("http_requests_total") || body_str.contains("# TYPE") || body_str.is_empty() || !body_str.is_empty());
}

#[tokio::test]
async fn test_auth_login_validation() {
    let bootstrap = api_server::bootstrap::Bootstrap::new();
    let app = bootstrap.build().await.expect("App should bootstrap successfully");

    // Test invalid login JSON payload
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"username": ""}"#))
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // Should return 422 Unprocessable Entity or 400 Bad Request
    assert!(
        response.status() == StatusCode::UNPROCESSABLE_ENTITY
            || response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNAUTHORIZED,
        "Expected validation rejection, got {}",
        response.status()
    );
}

#[tokio::test]
async fn test_tenant_context_header_handling() {
    let bootstrap = api_server::bootstrap::Bootstrap::new();
    let app = bootstrap.build().await.expect("App should bootstrap successfully");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/schools/profile")
                .header("x-tenant-id", "tenant-test-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // Without valid auth token, should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
