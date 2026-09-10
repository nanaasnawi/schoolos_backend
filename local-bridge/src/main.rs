use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use local_bridge::dapodik_acl::client::DapodikLocalClient;
use local_bridge::sync::engine::SyncEngine;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Deserialize)]
struct OnDemandSyncRequest {
    cloud_url: Option<String>,
    cloud_token: Option<String>,
    dapodik_url: Option<String>,
    npsn: Option<String>,
    dapodik_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    bridge: String,
    port: u16,
    dapodik_online: bool,
}

async fn health_handler() -> impl IntoResponse {
    // Probe Dapodik 5774
    let client = DapodikLocalClient::new(
        "http://127.0.0.1:5774".to_string(),
        "".to_string(),
        "".to_string(),
    );
    let dapodik_online = client.test_connection().await.is_ok();

    Json(ApiResponse {
        success: true,
        data: Some(HealthResponse {
            status: "ok".to_string(),
            bridge: "School OS Local Bridge Service".to_string(),
            port: 5775,
            dapodik_online,
        }),
        error: None,
    })
}

async fn sync_handler(
    Json(payload): Json<OnDemandSyncRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiResponse<()>>)> {
    let cloud_url = payload
        .cloud_url
        .unwrap_or_else(|| "https://schoolosbackend-production.up.railway.app".to_string());
    let cloud_token = payload.cloud_token.unwrap_or_default();
    let dapodik_url = payload
        .dapodik_url
        .unwrap_or_else(|| "http://127.0.0.1:5774".to_string());
    let npsn = payload.npsn.unwrap_or_default();
    let dapodik_token = payload.dapodik_token.unwrap_or_default();

    if npsn.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("NPSN sekolah wajib diisi.".to_string()),
            }),
        ));
    }

    match SyncEngine::perform_sync_params(
        &cloud_url,
        &cloud_token,
        &dapodik_url,
        &npsn,
        &dapodik_token,
    )
    .await
    {
        Ok(summary) => Ok(Json(ApiResponse {
            success: true,
            data: Some(summary),
            error: None,
        })),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(err.to_string()),
            }),
        )),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inisialisasi logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    // Permissive CORS layer allows requests from Vercel (https://school-os-academy.vercel.app)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/sync", post(sync_handler).get(health_handler))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 5775));
    info!("School OS Silent Bridge aktif di http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
