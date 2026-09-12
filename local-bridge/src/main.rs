#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use local_bridge::dapodik_acl::client::DapodikLocalClient;
use local_bridge::service::{
    is_running_from_install_dir, perform_self_install, show_native_message,
    unregister_windows_autostart,
};
use local_bridge::sync::engine::SyncEngine;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Deserialize)]
struct OnDemandSyncRequest {
    cloud_url: Option<String>,
    cloud_token: Option<String>,
    dapodik_url: Option<String>,
    npsn: Option<String>,
    dapodik_token: Option<String>,
    synced_by: Option<String>,
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
    let synced_by = payload.synced_by;

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
        synced_by.as_deref(),
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
    let args: Vec<String> = std::env::args().collect();
    let current_exe = std::env::current_exe()?;

    // 1. Uninstall flag: removes autostart & kills daemon
    if args.iter().any(|a| a == "--uninstall" || a == "-u") {
        match unregister_windows_autostart() {
            Ok(_) => {
                show_native_message(
                    "School OS Bridge",
                    "✓ School OS Bridge berhasil dicopot dan dinonaktifkan dari startup Windows.",
                    false,
                );
            }
            Err(e) => {
                show_native_message(
                    "School OS Bridge Error",
                    &format!("Gagal mencopot instalasi: {}", e),
                    true,
                );
            }
        }
        return Ok(());
    }

    // 2. Status query
    if args.iter().any(|a| a == "--status") {
        let is_running = reqwest::Client::new()
            .get("http://127.0.0.1:5775/health")
            .timeout(std::time::Duration::from_millis(1000))
            .send()
            .await
            .is_ok();

        if is_running {
            show_native_message(
                "School OS Bridge Status",
                "✓ School OS Bridge AKTIF dan berjalan di latar belakang (Port 5775).\n\nKomputer siap untuk sinkronisasi Dapodik.",
                false,
            );
        } else {
            show_native_message(
                "School OS Bridge Status",
                "✗ School OS Bridge TIDAK aktif.\n\nSilakan jalankan aplikasi schoolos-bridge.exe untuk mengaktifkannya.",
                true,
            );
        }
        return Ok(());
    }

    let is_daemon = args.iter().any(|a| a == "--daemon" || a == "-d");
    let is_force_install = args.iter().any(|a| a == "--install" || a == "-i");

    // 3. User interactive double-click check:
    if !is_daemon && !is_force_install {
        // Test if bridge is already running on port 5775
        let is_running = reqwest::Client::new()
            .get("http://127.0.0.1:5775/health")
            .timeout(std::time::Duration::from_millis(800))
            .send()
            .await
            .is_ok();

        if is_running && is_running_from_install_dir(&current_exe) {
            show_native_message(
                "School OS Bridge",
                "✓ School OS Bridge sudah aktif di latar belakang (Port 5775).\n\nAutostart komputer telah aktif. Anda siap melakukan tarik data Dapodik kapan saja.",
                false,
            );
            return Ok(());
        }
    }

    // 4. If not running in daemon mode and not in installed directory (or force install requested)
    if is_force_install || (!is_daemon && !is_running_from_install_dir(&current_exe)) {
        if let Err(e) = perform_self_install(&current_exe) {
            show_native_message(
                "School OS Bridge Setup Error",
                &format!(
                    "Gagal memasang School OS Bridge otomatis:\n{}\n\nSilakan coba jalankan sebagai Administrator.",
                    e
                ),
                true,
            );
        }
        return Ok(());
    }

    // 5. Run Axum server as background daemon
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    // Permissive CORS layer allows requests from Web Dashboard
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/sync", post(sync_handler).get(health_handler))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 5775));
    info!("School OS Silent Native Bridge aktif di http://{}", addr);

    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Bridge server stopped: {}", e);
            }
        }
        Err(e) => {
            error!("Gagal mengikat port {}: {}", addr, e);
        }
    }

    Ok(())
}
