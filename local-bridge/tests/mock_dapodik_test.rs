use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use local_bridge::dapodik_acl::adapter::DapodikAdapter;
use local_bridge::dapodik_acl::api_adapter::DapodikWebServiceAdapter;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct DapodikQueryParams {
    npsn: Option<String>,
}

// Mock Dapodik Handlers
async fn mock_get_sekolah(
    headers: HeaderMap,
    Query(params): Query<DapodikQueryParams>,
) -> impl IntoResponse {
    let auth = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if auth != "Bearer valid-dapodik-token" {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    if params.npsn.as_deref() != Some("12345678") {
        return (StatusCode::NOT_FOUND, "School Not Found").into_response();
    }

    Json(json!({
        "status": "success",
        "message": "Data sekolah ditemukan",
        "rows": [{
            "sekolah_id": "sch-001",
            "nama": "SMK Negeri 1 Test",
            "npsn": "12345678",
            "bentuk_pendidikan": "SMK"
        }]
    }))
    .into_response()
}

async fn mock_get_peserta_didik(
    headers: HeaderMap,
    Query(params): Query<DapodikQueryParams>,
) -> impl IntoResponse {
    let auth = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if auth != "Bearer valid-dapodik-token" {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    if params.npsn.as_deref() != Some("12345678") {
        return (StatusCode::BAD_REQUEST, "Invalid NPSN").into_response();
    }

    Json(json!({
        "status": "success",
        "rows": [
            {
                "registrasi_id": "reg-001",
                "peserta_didik_id": "pd-101",
                "nama": "Ahmad Dani",
                "nisn": "0081234567",
                "nik": "3201234567890001",
                "jenis_kelamin": "L",
                "tanggal_lahir": "2008-05-15",
                "tempat_lahir": "Jakarta",
                "rombel_saat_ini": "X-RPL-1"
            },
            {
                "registrasi_id": "reg-002",
                "peserta_didik_id": "pd-102",
                "nama": "Siti Rahma",
                "nisn": "0089876543",
                "nik": "3201234567890002",
                "jenis_kelamin": "P",
                "tanggal_lahir": "2008-08-20",
                "tempat_lahir": "Bandung",
                "rombel_saat_ini": "X-RPL-1"
            }
        ]
    }))
    .into_response()
}

// Spawn Mock Server Helper
async fn spawn_mock_dapodik_server() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/WebService/getSekolah", get(mock_get_sekolah))
        .route("/WebService/getPesertaDidik", get(mock_get_peserta_didik));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, handle)
}

#[tokio::test]
async fn test_dapodik_adapter_connect_success() {
    let (base_url, _server) = spawn_mock_dapodik_server().await;

    let adapter = DapodikWebServiceAdapter::new(
        base_url,
        "valid-dapodik-token".to_string(),
        "12345678".to_string(),
    )
    .expect("Adapter creation should succeed");

    let fingerprint = adapter.connect_and_fingerprint().await;
    assert!(fingerprint.is_ok());
    assert_eq!(fingerprint.unwrap(), "dapodik_api_v2027_compatible");
}

#[tokio::test]
async fn test_dapodik_adapter_unauthorized_token() {
    let (base_url, _server) = spawn_mock_dapodik_server().await;

    let adapter = DapodikWebServiceAdapter::new(
        base_url,
        "invalid-token-xyz".to_string(),
        "12345678".to_string(),
    )
    .expect("Adapter creation should succeed");

    let fingerprint = adapter.connect_and_fingerprint().await;
    assert!(fingerprint.is_err(), "Invalid token must return an error");
}

#[tokio::test]
async fn test_dapodik_adapter_get_students() {
    let (base_url, _server) = spawn_mock_dapodik_server().await;

    let adapter = DapodikWebServiceAdapter::new(
        base_url,
        "valid-dapodik-token".to_string(),
        "12345678".to_string(),
    )
    .expect("Adapter creation should succeed");

    let students = adapter
        .get_students(0_i64, 50_i32)
        .await
        .expect("Fetching students from mock Dapodik should succeed");

    assert_eq!(students.len(), 2);

    let first = &students[0];
    let second = &students[1];

    assert_eq!(first.full_name, "Ahmad Dani");
    assert_eq!(first.external_id, "pd-101");
    assert_eq!(second.full_name, "Siti Rahma");
    assert_eq!(second.external_id, "pd-102");
}
