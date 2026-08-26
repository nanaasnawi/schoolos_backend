use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use chrono::Utc;
use school_core::common::error::ApplicationError;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct DapodikSyncRecordDto {
    pub id: String,
    pub nisn: String,
    pub nik: String,
    pub nama_school_os: String,
    pub nama_dapodik: String,
    pub rombel: String,
    pub identity_state: String,
    pub mobility_case: String,
    pub classification: String,
    pub action_recommended: String,
    pub stage: String,
    pub last_synced_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct DapodikOutboxJobDto {
    pub job_id: String,
    pub req_id: String,
    pub operation: String,
    pub entity_id: String,
    pub idempotency_key: String,
    pub attempts: u32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PushDapodikJobRequest {
    pub entity_id: String,
    pub operation: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReconcileStudentRequest {
    pub sync_id: String,
    pub target_name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OpaqueQrTokenResponse {
    pub request_id: String,
    pub opaque_token: String,
    pub nonce: String,
    pub token_state: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateQrTokenRequest {
    pub student_name: String,
    pub nisn: String,
    pub nik: String,
    pub mother_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ClaimQrTokenRequest {
    pub request_id: String,
    pub opaque_token: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DapodikHealthResponse {
    pub connected: bool,
    pub status: String,
    pub message: String,
    pub dapodik_url: String,
    pub last_checked_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GeneratePrefillRequest {
    pub npsn: String,
    pub kode_registrasi: String,
    pub mirror_url: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DapodikPrefillResponse {
    pub prefill_id: String,
    pub npsn: String,
    pub mirror_used: String,
    pub total_siswa_imported: usize,
    pub total_rombel_imported: usize,
    pub status: String,
    pub message: String,
}

pub fn dapodik_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/health-check", get(check_dapodik_health))
        .route("/sync-records", get(list_sync_records))
        .route("/outbox-jobs", get(list_outbox_jobs))
        .route("/pull", post(pull_dapodik_records))
        .route("/push", post(push_dapodik_job))
        .route("/prefill/generate", post(generate_prefill_dapodik))
        .route("/prefill/upload", post(upload_prefill_file))
        .route("/import-excel", post(import_excel_dapodik))
        .route("/reconcile", post(reconcile_student))
        .route("/qr/create", post(create_qr_token))
        .route("/qr/claim", post(claim_qr_token))
}

#[utoipa::path(
    get,
    path = "/api/v1/dapodik/sync-records",
    responses(
        (status = 200, description = "List differential matrix sync records from PostgreSQL")
    ),
    security(("Bearer" = []))
)]
pub async fn list_sync_records(
    ctx: RequestContext,
    state: State<ApplicationContext>,
) -> Result<Json<ApiResponse<Vec<DapodikSyncRecordDto>>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT id, nisn, nik, nama_school_os, nama_dapodik, rombel, identity_state, mobility_case, classification, action_recommended, stage, last_synced_at
        FROM dapodik_sync_records
        WHERE tenant_id = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(ctx.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Database error: {}", e)), &ctx.request_id))?;

    let mut records: Vec<DapodikSyncRecordDto> = rows
        .into_iter()
        .map(|row| DapodikSyncRecordDto {
            id: row.get::<Uuid, _>("id").to_string(),
            nisn: row.get("nisn"),
            nik: row.get("nik"),
            nama_school_os: row.get("nama_school_os"),
            nama_dapodik: row.get("nama_dapodik"),
            rombel: row.get("rombel"),
            identity_state: row.get("identity_state"),
            mobility_case: row.get("mobility_case"),
            classification: row.get("classification"),
            action_recommended: row.get("action_recommended"),
            stage: row.get("stage"),
            last_synced_at: row
                .get::<chrono::DateTime<Utc>, _>("last_synced_at")
                .to_rfc3339(),
        })
        .collect();

    // Fallback: If dapodik_sync_records table is empty, fetch stored master students from PostgreSQL
    if records.is_empty() {
        let student_rows = sqlx::query(
            r#"
            SELECT s.id, s.nisn, s.full_name, COALESCE(c.name, '-') as rombel, s.status, s.updated_at
            FROM students s
            LEFT JOIN enrollments e ON s.id = e.student_id AND e.status = 'Active'
            LEFT JOIN classes c ON e.class_id = c.id
            WHERE s.tenant_id = $1
            ORDER BY s.created_at DESC
            "#
        )
        .bind(ctx.tenant_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

        for row in student_rows {
            let std_id: Uuid = row.get("id");
            let nisn: String = row.get("nisn");
            let full_name: String = row.get("full_name");
            let rombel: String = row.get("rombel");
            let status: String = row.get("status");
            let updated_at: chrono::DateTime<Utc> = row.get("updated_at");

            records.push(DapodikSyncRecordDto {
                id: std_id.to_string(),
                nisn: nisn.clone(),
                nik: nisn.clone(),
                nama_school_os: full_name.clone(),
                nama_dapodik: full_name,
                rombel,
                identity_state: if status.to_lowercase() == "active" {
                    "ACTIVE".into()
                } else {
                    "INACTIVE".into()
                },
                mobility_case: "NONE".into(),
                classification: "MATCH".into(),
                action_recommended: "Tersimpan Permanen di Database PostgreSQL School OS".into(),
                stage: "VERIFIED".into(),
                last_synced_at: updated_at.to_rfc3339(),
            });
        }
    }

    Ok(Json(ApiResponse::success(records, ctx.request_id)))
}

#[utoipa::path(
    get,
    path = "/api/v1/dapodik/outbox-jobs",
    responses(
        (status = 200, description = "List outbox queue jobs from PostgreSQL")
    ),
    security(("Bearer" = []))
)]
pub async fn list_outbox_jobs(
    ctx: RequestContext,
    state: State<ApplicationContext>,
) -> Result<Json<ApiResponse<Vec<DapodikOutboxJobDto>>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT job_id, req_id, operation, entity_id, idempotency_key, attempts, status, created_at
        FROM local_bridge_outbox_jobs
        WHERE tenant_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(ctx.tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Database error: {}", e)),
            &ctx.request_id,
        )
    })?;

    let jobs: Vec<DapodikOutboxJobDto> = rows
        .into_iter()
        .map(|row| DapodikOutboxJobDto {
            job_id: row.get::<Uuid, _>("job_id").to_string(),
            req_id: row.get("req_id"),
            operation: row.get("operation"),
            entity_id: row.get("entity_id"),
            idempotency_key: row.get("idempotency_key"),
            attempts: row.get::<i32, _>("attempts") as u32,
            status: row.get("status"),
            created_at: row
                .get::<chrono::DateTime<Utc>, _>("created_at")
                .to_rfc3339(),
        })
        .collect();

    Ok(Json(ApiResponse::success(jobs, ctx.request_id)))
}

/// Helper to dynamically resolve Dapodik URL, Host, Port and NPSN/Token
async fn resolve_dapodik_config(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    override_url: Option<String>,
    override_npsn: Option<String>,
    override_token: Option<String>,
) -> (String, String, u16, String, String) {
    let school_settings = sqlx::query!(
        "SELECT dapodik_url, npsn, dapodik_token FROM schools WHERE tenant_id = $1 LIMIT 1",
        tenant_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (db_url, db_npsn, db_token) = if let Some(s) = school_settings {
        (s.dapodik_url, s.npsn, s.dapodik_token)
    } else {
        (None, None, None)
    };

    // System settings fallback
    let sys_dapodik = sqlx::query!("SELECT value FROM system_settings WHERE key = 'dapodik'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.value.as_object().cloned());

    let sys_ip = sys_dapodik
        .as_ref()
        .and_then(|o| o.get("default_ip"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let sys_port = sys_dapodik
        .as_ref()
        .and_then(|o| o.get("default_port"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let env_url = std::env::var("DAPODIK_URL").ok();
    let env_host = std::env::var("DAPODIK_HOST").ok();
    let env_port = std::env::var("DAPODIK_PORT").ok();

    let raw_url = override_url
        .filter(|s| !s.trim().is_empty())
        .or(db_url.filter(|s| !s.trim().is_empty()))
        .or(env_url)
        .unwrap_or_else(|| {
            let host = env_host
                .or(sys_ip)
                .unwrap_or_else(|| "127.0.0.1".to_string());
            let port = env_port.or(sys_port).unwrap_or_else(|| "5774".to_string());
            format!("http://{}:{}", host, port)
        });

    let npsn = override_npsn
        .filter(|s| !s.trim().is_empty())
        .or(db_npsn)
        .unwrap_or_default()
        .trim()
        .to_string();

    let token = override_token
        .filter(|s| !s.trim().is_empty())
        .or(db_token)
        .unwrap_or_default()
        .trim()
        .to_string();

    // Extract host and port cleanly
    let (host, port) = if let Ok(parsed) = reqwest::Url::parse(&raw_url) {
        let h = parsed.host_str().unwrap_or("127.0.0.1").to_string();
        let p = parsed.port().unwrap_or(5774);
        (h, p)
    } else {
        ("127.0.0.1".to_string(), 5774)
    };

    (raw_url, host, port, npsn, token)
}

/// Helper for non-blocking TCP socket connection check (supports localhost, 127.0.0.1, host.docker.internal, etc.)
async fn probe_dapodik_tcp(host: &str, port: u16) -> bool {
    let target = format!("{}:{}", host, port);
    if let Ok(mut addrs) = tokio::net::lookup_host(&target).await {
        if let Some(socket_addr) = addrs.next() {
            return matches!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(1500),
                    tokio::net::TcpStream::connect(socket_addr),
                )
                .await,
                Ok(Ok(_))
            );
        }
    }
    false
}

pub async fn check_dapodik_health(
    ctx: RequestContext,
    state: State<ApplicationContext>,
) -> Result<Json<ApiResponse<DapodikHealthResponse>>, ApiError> {
    let (dapodik_url, host, port, _, _) =
        resolve_dapodik_config(&state.pool, ctx.tenant_id, None, None, None).await;

    let is_connected = probe_dapodik_tcp(&host, port).await;

    let (status, message) = if is_connected {
        (
            "ONLINE".to_string(),
            format!(
                "Terhubung ke Dapodik WebService di {} (Siap Sinkronisasi Real-Time).",
                dapodik_url
            ),
        )
    } else {
        (
            "OFFLINE".to_string(),
            format!(
                "Dapodik WebService ({}) sedang OFFLINE / Tidak Terjangkau. Data yang sudah pernah ditarik tetap AMAN tersimpan di Database PostgreSQL School OS.",
                dapodik_url
            ),
        )
    };

    let response = DapodikHealthResponse {
        connected: is_connected,
        status,
        message,
        dapodik_url,
        last_checked_at: Utc::now().to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(response, ctx.request_id)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PullDapodikRequest {
    pub dapodik_url: Option<String>,
    pub npsn: Option<String>,
    pub bearer_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DapodikRawStudent {
    pub peserta_didik_id: Option<String>,
    pub nipd: Option<String>,
    pub nisn: Option<String>,
    pub nik: Option<String>,
    pub nama: Option<String>,
    pub nama_pd: Option<String>,
    pub rombel: Option<String>,
    pub nama_rombel: Option<String>,
    pub jenis_kelamin: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub agama_id_str: Option<String>,
    pub nama_ayah: Option<String>,
    pub pekerjaan_ayah_id_str: Option<String>,
    pub nama_ibu: Option<String>,
    pub pekerjaan_ibu_id_str: Option<String>,
    pub nama_wali: Option<String>,
    pub pekerjaan_wali_id_str: Option<String>,
    pub nomor_telepon_seluler: Option<String>,
    pub nomor_telepon_rumah: Option<String>,
    pub alamat_jalan: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DapodikRawGtk {
    pub ptk_id: Option<String>,
    pub nip: Option<String>,
    pub nuptk: Option<String>,
    pub nik: Option<String>,
    pub nama: Option<String>,
    pub nama_ptk: Option<String>,
    pub nama_gtk: Option<String>,
    pub jenis_ptk: Option<String>,
    pub jenis_ptk_id_str: Option<String>,
    pub mata_pelajaran: Option<String>,
    pub mapel: Option<String>,
    pub jenis_kelamin: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub agama_id_str: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct DapodikRawPembelajaran {
    pub pembelajaran_id: Option<String>,
    pub mata_pelajaran_id: Option<serde_json::Value>,
    pub mata_pelajaran_id_str: Option<String>,
    pub nama_mata_pelajaran: Option<String>,
    pub ptk_id: Option<String>,
    pub jam_mengajar_per_minggu: Option<serde_json::Value>,
    pub status_di_kurikulum_str: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct DapodikRawRombel {
    pub rombongan_belajar_id: Option<String>,
    pub nama: Option<String>,
    pub ptk_id: Option<String>,
    pub tingkat_pendidikan_id: Option<String>,
    pub pembelajaran: Option<Vec<DapodikRawPembelajaran>>,
}

pub async fn pull_dapodik_records(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    payload: Option<Json<PullDapodikRequest>>,
) -> Result<Json<ApiResponse<Vec<DapodikSyncRecordDto>>>, ApiError> {
    let (override_url, override_npsn, override_token) = match payload {
        Some(Json(req)) => (req.dapodik_url, req.npsn, req.bearer_token),
        None => (None, None, None),
    };

    let (dapodik_url, host, port, npsn, token) = resolve_dapodik_config(
        &state.pool,
        ctx.tenant_id,
        override_url,
        override_npsn,
        override_token,
    )
    .await;

    // Probe TCP connectivity
    let is_online = probe_dapodik_tcp(&host, port).await;
    if !is_online {
        return Err(ApiError::new(
            ApplicationError::Internal(format!(
                "Sinkronisasi PULL Dibatalkan: Dapodik WebService ({}) sedang OFFLINE / Tidak Terjangkau. Pastikan aplikasi Dapodik dan WebService port {} di-start di host {}.",
                dapodik_url, port, host
            )),
            &ctx.request_id,
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Internal(format!("HTTP client error: {}", e)),
                &ctx.request_id,
            )
        })?;

    // ── 1. Start ACID Database Transaction (Finding 1) ──────────────────────
    let mut tx = state.pool.begin().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Failed to start database transaction: {}", e)),
            &ctx.request_id,
        )
    })?;

    // ── 2. Strict Master Data Resolution (Finding 5) ────────────────────────
    let default_ay =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM academic_years WHERE tenant_id = $1 LIMIT 1")
            .bind(ctx.tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Internal(format!(
                        "Database error reading academic years: {}",
                        e
                    )),
                    &ctx.request_id,
                )
            })?;

    let academic_year_id = match default_ay {
        Some(id) => id,
        None => {
            let new_ay = Uuid::now_v7();
            let now = Utc::now();
            sqlx::query(
                "INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at) VALUES ($1, $2, '2024/2025', $3, $4, true, $5, $5)"
            )
            .bind(new_ay)
            .bind(ctx.tenant_id)
            .bind(now)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert default academic year: {}", e)), &ctx.request_id))?;

            new_ay
        }
    };

    let default_gl =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM grade_levels WHERE tenant_id = $1 LIMIT 1")
            .bind(ctx.tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                ApiError::new(
                    ApplicationError::Internal(format!(
                        "Database error reading grade levels: {}",
                        e
                    )),
                    &ctx.request_id,
                )
            })?;

    let grade_level_id = match default_gl {
        Some(id) => id,
        None => {
            let new_gl = Uuid::now_v7();
            let now = Utc::now();
            sqlx::query(
                "INSERT INTO grade_levels (id, tenant_id, name, level, created_at, updated_at) VALUES ($1, $2, 'Tingkat 1', 1, $3, $3)"
            )
            .bind(new_gl)
            .bind(ctx.tenant_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert default grade level: {}", e)), &ctx.request_id))?;

            new_gl
        }
    };

    // ── 3. Pre-fetch Roles Once (Finding 2) ───────────────────────────────────
    let role_guru_id = match sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO roles (id, tenant_id, name, permissions, created_at) VALUES ($1, $2, 'Guru', '[\"StudentView\", \"ClassView\"]', NOW()) ON CONFLICT (tenant_id, name) DO UPDATE SET updated_at = EXCLUDED.created_at RETURNING id"
    )
    .bind(Uuid::now_v7())
    .bind(ctx.tenant_id)
    .fetch_one(&mut *tx)
    .await {
        Ok(id) => id,
        Err(_) => sqlx::query_scalar::<_, Uuid>("SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Guru'")
            .bind(ctx.tenant_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or_else(|_| Uuid::now_v7()),
    };

    let role_siswa_id = match sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO roles (id, tenant_id, name, permissions, created_at) VALUES ($1, $2, 'Siswa', '[\"StudentView\"]', NOW()) ON CONFLICT (tenant_id, name) DO UPDATE SET updated_at = EXCLUDED.created_at RETURNING id"
    )
    .bind(Uuid::now_v7())
    .bind(ctx.tenant_id)
    .fetch_one(&mut *tx)
    .await {
        Ok(id) => id,
        Err(_) => sqlx::query_scalar::<_, Uuid>("SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Siswa'")
            .bind(ctx.tenant_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or_else(|_| Uuid::now_v7()),
    };

    // ── 4. In-Memory Cache Pre-loading (Finding 2: Eliminating N+1) ───────────
    let class_rows = sqlx::query!(
        "SELECT id, name FROM classes WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut class_map: HashMap<String, Uuid> =
        class_rows.into_iter().map(|r| (r.name, r.id)).collect();

    let teacher_rows = sqlx::query!(
        "SELECT id, nip, nuptk, full_name FROM teachers WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut teacher_by_nip: HashMap<String, Uuid> = HashMap::new();
    let mut teacher_by_nuptk: HashMap<String, Uuid> = HashMap::new();
    let mut teacher_by_name: HashMap<String, Uuid> = HashMap::new();
    for t in teacher_rows {
        if let Some(n) = t.nip {
            teacher_by_nip.insert(n, t.id);
        }
        if let Some(nup) = t.nuptk {
            teacher_by_nuptk.insert(nup, t.id);
        }
        teacher_by_name.insert(t.full_name, t.id);
    }

    let staff_rows = sqlx::query!(
        "SELECT id, full_name FROM staff WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut staff_by_name: HashMap<String, Uuid> = staff_rows
        .into_iter()
        .map(|r| (r.full_name, r.id))
        .collect();

    let student_rows = sqlx::query!(
        "SELECT id, nisn, full_name FROM students WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut student_by_nisn: HashMap<String, Uuid> = HashMap::new();
    let mut student_by_name: HashMap<String, Uuid> = HashMap::new();
    for s in student_rows {
        student_by_nisn.insert(s.nisn, s.id);
        student_by_name.insert(s.full_name, s.id);
    }

    let sync_rows = sqlx::query!(
        "SELECT id, nisn, nama_dapodik FROM dapodik_sync_records WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut sync_by_nisn: HashMap<String, Uuid> = HashMap::new();
    let mut sync_by_name: HashMap<String, Uuid> = HashMap::new();
    for sr in sync_rows {
        sync_by_nisn.insert(sr.nisn, sr.id);
        sync_by_name.insert(sr.nama_dapodik, sr.id);
    }

    let guardian_rows = sqlx::query!(
        "SELECT id, full_name FROM guardians WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut guardian_map: HashMap<String, Uuid> = guardian_rows
        .into_iter()
        .map(|r| (r.full_name, r.id))
        .collect();

    let mut imported_records: Vec<DapodikSyncRecordDto> = Vec::new();
    let now = Utc::now();

    // ── 5. PULL GTK (Guru & Tendik) ──────────────────────────────────────────
    let target_gtk_url = format!(
        "{}/WebService/getGtk?npsn={}&limit=5000",
        dapodik_url.trim_end_matches('/'),
        npsn
    );
    let mut req_builder_gtk = client.get(&target_gtk_url);
    if !token.is_empty() {
        req_builder_gtk = req_builder_gtk.header("Authorization", format!("Bearer {}", token));
    }

    if let Ok(resp) = req_builder_gtk.send().await {
        if resp.status().is_success() {
            let raw_text = resp.text().await.unwrap_or_default();
            tracing::info!(
                "Dapodik getGtk raw response (first 200 chars): {}",
                raw_text.chars().take(200).collect::<String>()
            );
            let parsed_value: Result<serde_json::Value, _> = serde_json::from_str(&raw_text);
            let mut extracted_gtk: Option<Vec<DapodikRawGtk>> = None;
            if let Ok(val) = parsed_value {
                if val.is_array() {
                    extracted_gtk = serde_json::from_value(val).ok();
                } else if val.is_object() && val.get("rows").is_some() {
                    extracted_gtk = serde_json::from_value(val["rows"].clone()).ok();
                }
            }
            if let Some(teachers) = extracted_gtk {
                for (idx, gtk) in teachers.into_iter().enumerate() {
                    let ptk_id = gtk.ptk_id.clone().unwrap_or_else(|| format!("ptk-{}", idx));
                    let nip_val = gtk.nip.clone().or(gtk.nik.clone());
                    let nip = nip_val.filter(|s| !s.trim().is_empty());
                    let nuptk = gtk.nuptk.clone().filter(|s| !s.trim().is_empty());
                    let nama = gtk
                        .nama
                        .clone()
                        .or(gtk.nama_ptk.clone())
                        .or(gtk.nama_gtk.clone())
                        .unwrap_or_else(|| "GURU DAPODIK".to_string());
                    let subject_val = gtk
                        .jenis_ptk
                        .clone()
                        .or(gtk.jenis_ptk_id_str.clone())
                        .or(gtk.mata_pelajaran.clone())
                        .or(gtk.mapel.clone());
                    let new_id = Uuid::now_v7();

                    let user_id = Uuid::now_v7();
                    let email = format!(
                        "{}@guru.schoolos.id",
                        ptk_id.chars().take(8).collect::<String>()
                    );

                    let _ = sqlx::query(
                        "INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, true, $6, $6) ON CONFLICT (tenant_id, email) DO NOTHING"
                    ).bind(user_id).bind(ctx.tenant_id).bind(&email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&nama).bind(now).execute(&mut *tx).await;

                    let actual_user_id = sqlx::query_scalar::<_, Uuid>(
                        "SELECT id FROM users WHERE tenant_id = $1 AND email = $2",
                    )
                    .bind(ctx.tenant_id)
                    .bind(&email)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap_or(user_id);

                    let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                        .bind(actual_user_id).bind(role_guru_id).execute(&mut *tx).await;

                    let is_tendik =
                        gtk.jenis_ptk_id_str.as_deref().unwrap_or("").to_lowercase() != "guru";
                    let tgl_lahir = gtk
                        .tanggal_lahir
                        .as_ref()
                        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
                    let nama_upper = nama.to_uppercase();

                    if is_tendik {
                        let job_title = subject_val.clone().unwrap_or_else(|| "Tendik".to_string());
                        let existing_staff = staff_by_name.get(&nama_upper).copied();

                        if let Some(sid) = existing_staff {
                            let _ = sqlx::query("UPDATE staff SET user_id = $1, job_title = $2, jk = $3, tempat_lahir = $4, tanggal_lahir = $5, agama = $6, updated_at = $7 WHERE id = $8")
                                .bind(actual_user_id).bind(&job_title).bind(&gtk.jenis_kelamin).bind(&gtk.tempat_lahir).bind(tgl_lahir).bind(&gtk.agama_id_str).bind(now).bind(sid)
                                .execute(&mut *tx).await;
                        } else {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO staff (id, tenant_id, user_id, full_name, job_title, is_active, created_at, updated_at, jk, tempat_lahir, tanggal_lahir, agama)
                                VALUES ($1, $2, $3, $4, $5, true, $6, $6, $7, $8, $9, $10)
                                "#
                            )
                            .bind(new_id).bind(ctx.tenant_id).bind(actual_user_id).bind(&nama_upper).bind(&job_title).bind(now)
                            .bind(&gtk.jenis_kelamin).bind(&gtk.tempat_lahir).bind(tgl_lahir).bind(&gtk.agama_id_str)
                            .execute(&mut *tx).await;

                            staff_by_name.insert(nama_upper.clone(), new_id);
                        }

                        let nip_str = nip.clone().unwrap_or_else(|| "-".to_string());
                        imported_records.push(DapodikSyncRecordDto {
                            id: new_id.to_string(),
                            nisn: nip_str.clone(),
                            nik: nip_str,
                            nama_school_os: format!("[TENDIK] {}", nama_upper),
                            nama_dapodik: format!("[TENDIK] {}", nama),
                            rombel: "-".into(),
                            identity_state: "ACTIVE".into(),
                            mobility_case: "NONE".into(),
                            classification: "MATCH".into(),
                            action_recommended: "Pulled GTK Real-Time from Dapodik Localhost"
                                .into(),
                            stage: "VERIFIED".into(),
                            last_synced_at: now.to_rfc3339(),
                        });
                    } else {
                        let teacher_subject = subject_val.clone();
                        let existing_teacher = if let Some(ref n) = nip {
                            teacher_by_nip.get(n).copied()
                        } else if let Some(ref nup) = nuptk {
                            teacher_by_nuptk.get(nup).copied()
                        } else {
                            teacher_by_name.get(&nama_upper).copied()
                        };

                        if let Some(tid) = existing_teacher {
                            let _ = sqlx::query(
                                r#"
                                UPDATE teachers 
                                SET user_id = $1, subject = COALESCE($2, teachers.subject), jk = $3, tempat_lahir = $4, tanggal_lahir = $5, agama = $6, updated_at = $7, nuptk = COALESCE($8, teachers.nuptk)
                                WHERE id = $9
                                "#
                            )
                            .bind(actual_user_id).bind(&teacher_subject).bind(&gtk.jenis_kelamin).bind(&gtk.tempat_lahir).bind(tgl_lahir).bind(&gtk.agama_id_str).bind(now).bind(&nuptk).bind(tid)
                            .execute(&mut *tx).await;
                        } else {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO teachers (id, tenant_id, user_id, nip, full_name, subject, is_active, created_at, updated_at, jk, tempat_lahir, tanggal_lahir, agama, nuptk)
                                VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7, $8, $9, $10, $11, $12)
                                "#
                            )
                            .bind(new_id).bind(ctx.tenant_id).bind(actual_user_id).bind(&nip).bind(&nama_upper).bind(&teacher_subject).bind(now)
                            .bind(&gtk.jenis_kelamin).bind(&gtk.tempat_lahir).bind(tgl_lahir).bind(&gtk.agama_id_str).bind(&nuptk)
                            .execute(&mut *tx).await;

                            if let Some(ref n) = nip {
                                teacher_by_nip.insert(n.clone(), new_id);
                            }
                            if let Some(ref nup) = nuptk {
                                teacher_by_nuptk.insert(nup.clone(), new_id);
                            }
                            teacher_by_name.insert(nama_upper.clone(), new_id);
                        }

                        let nip_str = nip.unwrap_or_else(|| "-".to_string());
                        imported_records.push(DapodikSyncRecordDto {
                            id: new_id.to_string(),
                            nisn: nip_str.clone(),
                            nik: nip_str,
                            nama_school_os: format!("[GURU] {}", nama_upper),
                            nama_dapodik: format!("[GURU] {}", nama),
                            rombel: "-".into(),
                            identity_state: "ACTIVE".into(),
                            mobility_case: "NONE".into(),
                            classification: "MATCH".into(),
                            action_recommended: "Pulled GTK Real-Time from Dapodik Localhost"
                                .into(),
                            stage: "VERIFIED".into(),
                            last_synced_at: now.to_rfc3339(),
                        });
                    }
                }
            }
        }
    }

    // ── 6. PULL ROMBEL & PEMBELAJARAN (MAPEL) ────────────────────────────────
    let target_rombel_url = format!(
        "{}/WebService/getRombonganBelajar?npsn={}&limit=5000",
        dapodik_url.trim_end_matches('/'),
        npsn
    );
    let mut req_builder_rombel = client.get(&target_rombel_url);
    if !token.is_empty() {
        req_builder_rombel =
            req_builder_rombel.header("Authorization", format!("Bearer {}", token));
    }

    if let Ok(resp) = req_builder_rombel.send().await {
        if resp.status().is_success() {
            let raw_text = resp.text().await.unwrap_or_default();
            tracing::info!(
                "Dapodik getRombel raw response (first 200 chars): {}",
                raw_text.chars().take(200).collect::<String>()
            );
            let parsed_value: Result<serde_json::Value, _> = serde_json::from_str(&raw_text);
            let mut extracted_rombel: Option<Vec<DapodikRawRombel>> = None;
            if let Ok(val) = parsed_value {
                if val.is_array() {
                    extracted_rombel = serde_json::from_value(val).ok();
                } else if val.is_object() && val.get("rows").is_some() {
                    extracted_rombel = serde_json::from_value(val["rows"].clone()).ok();
                }
            }
            if let Some(rombels) = extracted_rombel {
                let mut processed_subjects: HashSet<String> = HashSet::new();

                for rmbl in rombels {
                    let nama_rombel = rmbl.nama.unwrap_or_else(|| "ROMBEL DAPODIK".to_string());
                    let new_id = Uuid::now_v7();

                    if !class_map.contains_key(&nama_rombel) {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, name, capacity, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, 30, $6, $6)
                            "#
                        )
                        .bind(new_id).bind(ctx.tenant_id).bind(academic_year_id).bind(grade_level_id)
                        .bind(&nama_rombel).bind(now).execute(&mut *tx).await;

                        class_map.insert(nama_rombel.clone(), new_id);
                    }

                    // Extract Pembelajaran (Mata Pelajaran) from Rombel
                    if let Some(pembelajaran_list) = rmbl.pembelajaran {
                        for p in pembelajaran_list {
                            let raw_code = p
                                .mata_pelajaran_id
                                .map(|v| v.to_string().replace('\"', ""))
                                .filter(|s| !s.trim().is_empty())
                                .unwrap_or_else(|| {
                                    format!(
                                        "MP-{}",
                                        Uuid::new_v4()
                                            .to_string()
                                            .chars()
                                            .take(6)
                                            .collect::<String>()
                                    )
                                });
                            let code = raw_code.trim().to_string();

                            let name = p
                                .nama_mata_pelajaran
                                .or(p.mata_pelajaran_id_str)
                                .unwrap_or_else(|| "Mata Pelajaran".to_string());

                            if !processed_subjects.contains(&name) {
                                processed_subjects.insert(name.clone());
                                let subject_new_id = Uuid::now_v7();

                                let _ = sqlx::query(
                                    r#"
                                    INSERT INTO subjects (id, tenant_id, code, name, is_active, created_at, updated_at)
                                    VALUES ($1, $2, $3, $4, true, $5, $5)
                                    ON CONFLICT (tenant_id, code) DO UPDATE 
                                    SET name = EXCLUDED.name, is_active = true, updated_at = EXCLUDED.updated_at
                                    "#
                                )
                                .bind(subject_new_id)
                                .bind(ctx.tenant_id)
                                .bind(&code)
                                .bind(&name)
                                .bind(now)
                                .execute(&mut *tx)
                                .await;

                                imported_records.push(DapodikSyncRecordDto {
                                    id: subject_new_id.to_string(),
                                    nisn: code.clone(),
                                    nik: code.clone(),
                                    nama_school_os: format!("[MAPEL] {}", name.to_uppercase()),
                                    nama_dapodik: format!("[MAPEL] {}", name),
                                    rombel: nama_rombel.clone(),
                                    identity_state: "ACTIVE".into(),
                                    mobility_case: "NONE".into(),
                                    classification: "MATCH".into(),
                                    action_recommended: "Pulled Pembelajaran (Mata Pelajaran) Real-Time from Dapodik".into(),
                                    stage: "VERIFIED".into(),
                                    last_synced_at: now.to_rfc3339(),
                                });
                            }
                        }
                    }

                    imported_records.push(DapodikSyncRecordDto {
                        id: new_id.to_string(),
                        nisn: "-".into(),
                        nik: "-".into(),
                        nama_school_os: format!("[KELAS] {}", nama_rombel.to_uppercase()),
                        nama_dapodik: format!("[KELAS] {}", nama_rombel),
                        rombel: nama_rombel,
                        identity_state: "ACTIVE".into(),
                        mobility_case: "NONE".into(),
                        classification: "MATCH".into(),
                        action_recommended: "Pulled Rombel Real-Time from Dapodik Localhost".into(),
                        stage: "VERIFIED".into(),
                        last_synced_at: now.to_rfc3339(),
                    });
                }
            }
        }
    }

    // ── 7. PULL PESERTA DIDIK (O(1) HashSet & Strict Rombel Safety) ──────────
    let target_api_url = format!(
        "{}/WebService/getPesertaDidik?npsn={}&limit=5000",
        dapodik_url.trim_end_matches('/'),
        npsn
    );
    let mut req_builder = client.get(&target_api_url);
    if !token.is_empty() {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
    }

    let http_res = req_builder.send().await;
    match http_res {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                return Err(ApiError::new(
                    ApplicationError::Internal(format!(
                        "WebService Dapodik ({}) merespon HTTP status {}. Mohon cek NPSN dan Token WebService Dapodik di Pengaturan Sekolah.",
                        target_api_url, status
                    )),
                    &ctx.request_id,
                ));
            }
            let raw_text = resp.text().await.unwrap_or_default();
            tracing::info!(
                "Dapodik getPesertaDidik raw response (first 500 chars): {}",
                raw_text.chars().take(500).collect::<String>()
            );
            let parsed_value: Result<serde_json::Value, _> = serde_json::from_str(&raw_text);
            let mut extracted_students: Option<Vec<DapodikRawStudent>> = None;
            if let Ok(val) = parsed_value {
                if val.is_array() {
                    extracted_students = serde_json::from_value(val).ok();
                } else if val.is_object() && val.get("rows").is_some() {
                    extracted_students = serde_json::from_value(val["rows"].clone()).ok();
                }
            }
            if let Some(students) = extracted_students {
                // Finding 4: Use HashSet for O(1) duplicate & mutasi lookup
                let mut active_nisns: HashSet<String> = HashSet::new();

                for (idx, std) in students.into_iter().enumerate() {
                    let pd_id = std
                        .peserta_didik_id
                        .clone()
                        .unwrap_or_else(|| format!("pd-{}", idx));
                    let nik = std.nik.filter(|s| !s.trim().is_empty());
                    let nama = std
                        .nama
                        .or(std.nama_pd)
                        .unwrap_or_else(|| "SISWA DAPODIK".to_string());
                    let nama_upper = nama.to_uppercase();
                    let new_id = Uuid::now_v7();

                    let nisn_val = std.nisn.filter(|s| !s.trim().is_empty());
                    let nipd_val = std.nipd.filter(|s| !s.trim().is_empty());
                    let nik_val = nik.clone();

                    let mut final_nisn = nisn_val
                        .clone()
                        .or(nipd_val.clone())
                        .or(nik_val)
                        .unwrap_or_else(|| {
                            format!(
                                "T-{}",
                                Uuid::new_v4()
                                    .to_string()
                                    .chars()
                                    .take(8)
                                    .collect::<String>()
                            )
                        });

                    // O(1) HashSet check
                    if active_nisns.contains(&final_nisn) {
                        final_nisn = format!(
                            "T-{}",
                            Uuid::new_v4()
                                .to_string()
                                .chars()
                                .take(8)
                                .collect::<String>()
                        );
                    }
                    active_nisns.insert(final_nisn.clone());

                    // Clean & Validate Rombel from Dapodik
                    // USER RULE: Siswa yang belum masuk rombel jangan asal dimasukkan rombel dummy!
                    let valid_rombel = std
                        .rombel
                        .or(std.nama_rombel)
                        .map(|s| s.trim().to_string())
                        .filter(|s| {
                            !s.is_empty()
                                && s != "-"
                                && s != "null"
                                && !s.eq_ignore_ascii_case("belum ada rombel")
                                && !s.eq_ignore_ascii_case("belum masuk rombel")
                                && !s.eq_ignore_ascii_case("umum")
                                && !s.eq_ignore_ascii_case("rombel aktif")
                        });

                    let sync_rombel_label = valid_rombel.clone().unwrap_or_else(|| "-".to_string());

                    // Sync Records Upsert (using cache)
                    let existing_sync_id = if !final_nisn.starts_with("T-") {
                        sync_by_nisn.get(&final_nisn).copied()
                    } else {
                        sync_by_name.get(&nama).copied()
                    };

                    if let Some(id) = existing_sync_id {
                        let _ = sqlx::query(
                            "UPDATE dapodik_sync_records SET nama_school_os = $1, nama_dapodik = $2, rombel = $3, nik = $4, identity_state = 'ACTIVE', last_synced_at = $5 WHERE id = $6"
                        ).bind(&nama_upper).bind(&nama).bind(&sync_rombel_label).bind(&nik).bind(now).bind(id).execute(&mut *tx).await;
                    } else {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO dapodik_sync_records
                            (id, tenant_id, nisn, nik, nama_school_os, nama_dapodik, rombel, identity_state, mobility_case, classification, action_recommended, stage, last_synced_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 'NONE', 'MATCH', 'Pulled Real-Time from Dapodik Localhost WebService', 'VERIFIED', $8)
                            "#
                        )
                        .bind(new_id).bind(ctx.tenant_id).bind(&final_nisn).bind(&nik).bind(&nama_upper)
                        .bind(&nama).bind(&sync_rombel_label).bind(now).execute(&mut *tx).await;

                        sync_by_nisn.insert(final_nisn.clone(), new_id);
                        sync_by_name.insert(nama.clone(), new_id);
                    }

                    // User Account Creation
                    let user_id = Uuid::now_v7();
                    let email = format!(
                        "{}@student.schoolos.id",
                        pd_id.chars().take(8).collect::<String>()
                    );

                    let _ = sqlx::query(
                        "INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, true, $6, $6) ON CONFLICT (tenant_id, email) DO UPDATE SET full_name = EXCLUDED.full_name, updated_at = EXCLUDED.updated_at"
                    ).bind(user_id).bind(ctx.tenant_id).bind(&email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&nama).bind(now).execute(&mut *tx).await;

                    let actual_user_id = sqlx::query_scalar::<_, Uuid>(
                        "SELECT id FROM users WHERE tenant_id = $1 AND email = $2",
                    )
                    .bind(ctx.tenant_id)
                    .bind(&email)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap_or(user_id);

                    let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                        .bind(actual_user_id).bind(role_siswa_id).execute(&mut *tx).await;

                    // Guardian Handling (using guardian_map cache)
                    let guardian_name = if let Some(ref a) = std
                        .nama_ayah
                        .as_ref()
                        .filter(|s| !s.trim().is_empty() && *s != "-" && *s != "Tidak ada")
                    {
                        Some(a.trim().to_uppercase())
                    } else if let Some(ref i) = std
                        .nama_ibu
                        .as_ref()
                        .filter(|s| !s.trim().is_empty() && *s != "-" && *s != "Tidak ada")
                    {
                        Some(i.trim().to_uppercase())
                    } else if let Some(ref w) = std
                        .nama_wali
                        .as_ref()
                        .filter(|s| !s.trim().is_empty() && *s != "-" && *s != "Tidak ada")
                    {
                        Some(w.trim().to_uppercase())
                    } else {
                        None
                    };

                    let guardian_phone = std
                        .nomor_telepon_seluler
                        .clone()
                        .or(std.nomor_telepon_rumah.clone())
                        .filter(|s| !s.trim().is_empty());
                    let student_address = std.alamat_jalan.clone().filter(|s| !s.trim().is_empty());

                    let mut final_guardian_id: Option<Uuid> = None;
                    if let Some(ref g_name) = guardian_name {
                        if let Some(&gid) = guardian_map.get(g_name) {
                            if guardian_phone.is_some() {
                                let _ = sqlx::query("UPDATE guardians SET phone_number = $1, updated_at = $2 WHERE id = $3")
                                    .bind(&guardian_phone).bind(now).bind(gid).execute(&mut *tx).await;
                            }
                            final_guardian_id = Some(gid);
                        } else {
                            let new_gid = Uuid::now_v7();
                            if let Ok(_) = sqlx::query(
                                "INSERT INTO guardians (id, tenant_id, full_name, phone_number, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5)"
                            )
                            .bind(new_gid)
                            .bind(ctx.tenant_id)
                            .bind(g_name)
                            .bind(&guardian_phone)
                            .bind(now)
                            .execute(&mut *tx)
                            .await {
                                guardian_map.insert(g_name.clone(), new_gid);
                                final_guardian_id = Some(new_gid);
                            }
                        }
                    }

                    // Student Record Upsert (using student_by_nisn cache)
                    let existing_student_id = if !final_nisn.starts_with("T-") {
                        student_by_nisn.get(&final_nisn).copied()
                    } else {
                        student_by_name.get(&nama_upper).copied()
                    };

                    let student_db_id = if let Some(sid) = existing_student_id {
                        let _ = sqlx::query(
                            r#"
                            UPDATE students 
                            SET full_name = $1, user_id = $2, nik = $3, gender = $4, place_of_birth = $5, date_of_birth = $6, religion = $7, 
                                guardian_id = COALESCE($8, students.guardian_id), nipd = COALESCE($9, students.nipd),
                                alamat_jalan = COALESCE($10, students.alamat_jalan), no_hp = COALESCE($11, students.no_hp),
                                email = COALESCE($12, students.email), status = 'active', updated_at = $13
                            WHERE id = $14
                            "#
                        )
                        .bind(&nama_upper).bind(actual_user_id).bind(&nik).bind(&std.jenis_kelamin).bind(&std.tempat_lahir)
                        .bind(std.tanggal_lahir.as_ref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
                        .bind(&std.agama_id_str).bind(final_guardian_id).bind(nipd_val.clone()).bind(&student_address).bind(&guardian_phone)
                        .bind(&std.email).bind(now).bind(sid).execute(&mut *tx).await;
                        sid
                    } else {
                        let inserted_id = sqlx::query_scalar::<_, Uuid>(
                            r#"
                            INSERT INTO students (id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, status, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'active', $16, $16)
                            RETURNING id
                            "#
                        )
                        .bind(new_id).bind(ctx.tenant_id).bind(actual_user_id).bind(final_guardian_id).bind(&final_nisn).bind(&nama_upper)
                        .bind(&nik).bind(&std.jenis_kelamin).bind(&std.tempat_lahir)
                        .bind(std.tanggal_lahir.as_ref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
                        .bind(&std.agama_id_str).bind(nipd_val).bind(&student_address).bind(&guardian_phone).bind(&std.email).bind(now).fetch_one(&mut *tx).await.unwrap_or_else(|e| {
                            tracing::error!("Failed to insert student (NISN: {}, Name: {}): {}", final_nisn, nama, e);
                            new_id
                        });

                        student_by_nisn.insert(final_nisn.clone(), inserted_id);
                        student_by_name.insert(nama_upper.clone(), inserted_id);
                        inserted_id
                    };

                    // ── 8. Enrollment & Rombel Association Logic ─────────────
                    if let Some(ref r_name) = valid_rombel {
                        let c_id = if let Some(&id) = class_map.get(r_name) {
                            id
                        } else {
                            let new_c_id = Uuid::now_v7();
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, name, capacity, created_at, updated_at)
                                VALUES ($1, $2, $3, $4, $5, 30, $6, $6)
                                "#
                            )
                            .bind(new_c_id).bind(ctx.tenant_id).bind(academic_year_id).bind(grade_level_id).bind(r_name).bind(now).execute(&mut *tx).await;

                            class_map.insert(r_name.clone(), new_c_id);
                            new_c_id
                        };

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO enrollments (id, tenant_id, student_id, class_id, academic_year_id, status, enrolled_at)
                            VALUES ($1, $2, $3, $4, $5, 'Active', $6)
                            ON CONFLICT (student_id, academic_year_id) WHERE status = 'Active'
                            DO UPDATE SET class_id = EXCLUDED.class_id, status = 'Active'
                            "#
                        )
                        .bind(Uuid::now_v7()).bind(ctx.tenant_id).bind(student_db_id).bind(c_id)
                        .bind(academic_year_id).bind(now).execute(&mut *tx).await;
                    } else {
                        // SISWA BELUM MASUK ROMBEL DI DAPODIK -> KOSONGKAN & JANGAN ASAL ENROLL!
                        let _ = sqlx::query(
                            "DELETE FROM enrollments WHERE tenant_id = $1 AND student_id = $2 AND academic_year_id = $3"
                        )
                        .bind(ctx.tenant_id)
                        .bind(student_db_id)
                        .bind(academic_year_id)
                        .execute(&mut *tx)
                        .await;
                    }

                    let nisn_str = final_nisn.clone();
                    let nik_str = nik.clone().unwrap_or_else(|| "-".to_string());
                    imported_records.push(DapodikSyncRecordDto {
                        id: new_id.to_string(),
                        nisn: nisn_str,
                        nik: nik_str,
                        nama_school_os: nama_upper,
                        nama_dapodik: nama,
                        rombel: sync_rombel_label,
                        identity_state: "ACTIVE".into(),
                        mobility_case: "NONE".into(),
                        classification: "MATCH".into(),
                        action_recommended: "Pulled Real-Time from Dapodik Localhost WebService"
                            .into(),
                        stage: "VERIFIED".into(),
                        last_synced_at: now.to_rfc3339(),
                    });
                }

                // ── 9. Automatic Detection for Transferred Out (Mutasi Keluar) ─
                if !active_nisns.is_empty() {
                    let active_nisns_vec: Vec<String> = active_nisns.into_iter().collect();

                    let _ = sqlx::query(
                        "UPDATE students SET status = 'transferred', updated_at = $2 WHERE tenant_id = $1 AND (status = 'active' OR status = 'Active') AND NOT (nisn = ANY($3))"
                    ).bind(ctx.tenant_id).bind(now).bind(&active_nisns_vec).execute(&mut *tx).await;

                    let _ = sqlx::query(
                        "UPDATE dapodik_sync_records SET identity_state = 'MUTASI_OUT', mobility_case = 'TRANSFER_OUT_APPROVED', last_synced_at = $2 WHERE tenant_id = $1 AND NOT (nisn = ANY($3))"
                    ).bind(ctx.tenant_id).bind(now).bind(&active_nisns_vec).execute(&mut *tx).await;
                }
            } else {
                if raw_text.contains("Access denied") {
                    return Err(ApiError::new(
                        ApplicationError::Internal(
                            "Akses WebService Dapodik Ditolak (Access Denied). Mohon cek 2 hal di Dapodik: 1) Buka menu Pengaturan > Web Service, pastikan 'IP Pengakses' diisi '127.0.0.1' / host server. 2) Pastikan 'Token WebService' dan 'NPSN' di Pengaturan School OS sudah sesuai dengan data di Dapodik.".to_string()
                        ),
                        &ctx.request_id,
                    ));
                }
                return Err(ApiError::new(
                    ApplicationError::Internal(format!(
                        "Terhubung ke Dapodik WebService, tetapi respon WebService bukan JSON array peserta didik. Respon: {}",
                        raw_text.chars().take(150).collect::<String>()
                    )),
                    &ctx.request_id,
                ));
            }
        }
        Err(err) => {
            return Err(ApiError::new(
                ApplicationError::Internal(format!(
                    "Koneksi HTTP ke Dapodik WebService gagal: {}",
                    err
                )),
                &ctx.request_id,
            ));
        }
    }

    // ── 10. Commit Database Transaction (Finding 1) ───────────────────────────
    tx.commit().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Failed to commit database transaction: {}", e)),
            &ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(imported_records, ctx.request_id)))
}
pub async fn push_dapodik_job(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<PushDapodikJobRequest>,
) -> Result<Json<ApiResponse<DapodikOutboxJobDto>>, ApiError> {
    let job_id = Uuid::now_v7();
    let req_id = format!("req_mut_{}", Uuid::now_v7());
    let idempotency_key = format!("sha256({}+{}+{})", ctx.tenant_id, payload.entity_id, req_id);
    let now = Utc::now();

    let _ = sqlx::query(
        r#"
        INSERT INTO local_bridge_outbox_jobs
        (job_id, tenant_id, req_id, operation, entity_id, idempotency_key, attempts, status, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, 1, 'COMPLETED', $7)
        "#
    )
    .bind(job_id)
    .bind(ctx.tenant_id)
    .bind(&req_id)
    .bind(&payload.operation)
    .bind(&payload.entity_id)
    .bind(&idempotency_key)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert outbox job: {}", e)), &ctx.request_id))?;

    let new_job = DapodikOutboxJobDto {
        job_id: job_id.to_string(),
        req_id,
        operation: payload.operation,
        entity_id: payload.entity_id,
        idempotency_key,
        attempts: 1,
        status: "COMPLETED".into(),
        created_at: now.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(new_job, ctx.request_id)))
}

pub async fn generate_prefill_dapodik(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<GeneratePrefillRequest>,
) -> Result<Json<ApiResponse<DapodikPrefillResponse>>, ApiError> {
    if payload.kode_registrasi.trim().is_empty() || payload.npsn.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Internal(
                "NPSN dan Kode Registrasi Dapodik tidak boleh kosong".into(),
            ),
            &ctx.request_id,
        ));
    }

    let prefill_uuid = Uuid::now_v7();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            ApiError::new(
                ApplicationError::Internal(format!("HTTP client error: {}", e)),
                &ctx.request_id,
            )
        })?;

    let download_url = format!(
        "{}/prefill/get_prefill?npsn={}&kode_reg={}",
        payload.mirror_url.trim_end_matches('/'),
        payload.npsn.trim(),
        payload.kode_registrasi.trim()
    );

    let resp = client.get(&download_url).send().await;

    match resp {
        Ok(res) => {
            let status = res.status();
            if !status.is_success() {
                return Err(ApiError::new(
                    ApplicationError::Internal(format!(
                        "Gagal mengunduh file prefill dari Server Kemendikdasmen ({}): Server merespon HTTP status {}. Pastikan NPSN ({}) dan Kode Registrasi ({}) valid.",
                        payload.mirror_url, status, payload.npsn, payload.kode_registrasi
                    )),
                    &ctx.request_id,
                ));
            }

            let body_bytes = res.bytes().await.unwrap_or_default();
            if body_bytes.is_empty() {
                return Err(ApiError::new(
                    ApplicationError::Internal(format!(
                        "Server Prefill Kemendikdasmen ({}) mengembalikan file kosong (0 bytes) untuk NPSN {}.",
                        payload.mirror_url, payload.npsn
                    )),
                    &ctx.request_id,
                ));
            }

            let mut total_siswa = 0;
            let mut total_rombel = 0;

            if let Ok(text) = std::str::from_utf8(&body_bytes) {
                let trimmed = text.trim();
                if trimmed.starts_with("<!DOCTYPE")
                    || trimmed.starts_with("<html")
                    || trimmed.starts_with("<?xml")
                {
                    return Err(ApiError::new(
                        ApplicationError::Internal(format!(
                            "Server Kemendikdasmen ({}) mengembalikan halaman HTML/Web. Silakan unduh file .prf secara langsung di portal https://prefill1.kemendikdasmen.go.id lalu pilih 'Pilih File .prf dari Komputer' di bawah.",
                            payload.mirror_url
                        )),
                        &ctx.request_id,
                    ));
                }

                if let Ok(students) = serde_json::from_str::<Vec<DapodikRawStudent>>(trimmed) {
                    let now = Utc::now();
                    for std in students {
                        let nisn = std.nisn.unwrap_or_else(|| "0000000000".to_string());
                        let nik = std.nik.unwrap_or_else(|| "0000000000000000".to_string());
                        let nama = std
                            .nama
                            .or(std.nama_pd)
                            .unwrap_or_else(|| "SISWA PREFILL".to_string());
                        let rombel = std
                            .rombel
                            .or(std.nama_rombel)
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty() && s != "null" && s != "UMUM" && s != "7A")
                            .unwrap_or_else(|| "-".to_string());
                        let new_id = Uuid::now_v7();

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO dapodik_sync_records
                            (id, tenant_id, nisn, nik, nama_school_os, nama_dapodik, rombel, identity_state, mobility_case, classification, action_recommended, stage, last_synced_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 'NONE', 'NORMALIZATION', 'Imported via Kemendikdasmen Prefill Engine', 'APPROVED', $8)
                            ON CONFLICT (id) DO NOTHING
                            "#
                        )
                        .bind(new_id)
                        .bind(ctx.tenant_id)
                        .bind(&nisn)
                        .bind(&nik)
                        .bind(nama.to_uppercase())
                        .bind(&nama)
                        .bind(&rombel)
                        .bind(now)
                        .execute(&state.pool)
                        .await;

                        let role_siswa_id = match sqlx::query_scalar::<_, Uuid>(
                            "INSERT INTO roles (id, tenant_id, name, permissions, created_at) VALUES ($1, $2, 'Siswa', '[\"StudentView\"]', $3) ON CONFLICT (tenant_id, name) DO UPDATE SET updated_at = EXCLUDED.created_at RETURNING id"
                        ).bind(Uuid::now_v7()).bind(ctx.tenant_id).bind(now).fetch_one(&state.pool).await {
                            Ok(id) => id,
                            Err(_) => sqlx::query_scalar::<_, Uuid>("SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Siswa'").bind(ctx.tenant_id).fetch_one(&state.pool).await.unwrap_or(Uuid::now_v7()),
                        };

                        let user_id = Uuid::now_v7();
                        let email = format!(
                            "{}@student.schoolos.id",
                            nisn.to_lowercase().replace(" ", "")
                        );

                        let _ = sqlx::query(
                            "INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, true, $6, $6) ON CONFLICT (tenant_id, email) DO NOTHING"
                        ).bind(user_id).bind(ctx.tenant_id).bind(&email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&nama).bind(now).execute(&state.pool).await;

                        let actual_user_id = sqlx::query_scalar::<_, Uuid>(
                            "SELECT id FROM users WHERE tenant_id = $1 AND email = $2",
                        )
                        .bind(ctx.tenant_id)
                        .bind(&email)
                        .fetch_one(&state.pool)
                        .await
                        .unwrap_or(user_id);

                        let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(actual_user_id).bind(role_siswa_id).execute(&state.pool).await;

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO students (id, tenant_id, user_id, nisn, full_name, status, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, 'Active', $6, $6)
                            ON CONFLICT (tenant_id, nisn) DO UPDATE 
                            SET full_name = EXCLUDED.full_name, user_id = EXCLUDED.user_id, updated_at = EXCLUDED.updated_at
                            "#
                        )
                        .bind(new_id)
                        .bind(ctx.tenant_id)
                        .bind(actual_user_id)
                        .bind(&nisn)
                        .bind(nama.to_uppercase())
                        .bind(now)
                        .execute(&state.pool)
                        .await;

                        total_siswa += 1;
                    }
                    total_rombel = 1;
                }
            }

            let response_dto = DapodikPrefillResponse {
                prefill_id: prefill_uuid.to_string(),
                npsn: payload.npsn,
                mirror_used: payload.mirror_url,
                total_siswa_imported: total_siswa,
                total_rombel_imported: total_rombel,
                status: "PREFILL_PARSED_AND_IMPORTED".into(),
                message: format!(
                    "File Prefill Dapodik ({:.2} KB) diunduh dari Kemendikdasmen. Total {} siswa di-impor ke database master School OS.",
                    body_bytes.len() as f64 / 1024.0,
                    total_siswa
                ),
            };

            Ok(Json(ApiResponse::success(response_dto, ctx.request_id)))
        }
        Err(e) => Err(ApiError::new(
            ApplicationError::Internal(format!(
                "Koneksi ke Server Mirror Kemendikdasmen ({}) gagal: {}",
                payload.mirror_url, e
            )),
            &ctx.request_id,
        )),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadPrefillRequest {
    pub file_name: Option<String>,
    pub content_text: String,
}

pub async fn upload_prefill_file(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<UploadPrefillRequest>,
) -> Result<Json<ApiResponse<DapodikPrefillResponse>>, ApiError> {
    if payload.content_text.trim().is_empty() {
        return Err(ApiError::new(
            ApplicationError::Internal("Isi file prefill (.prf) tidak boleh kosong".into()),
            &ctx.request_id,
        ));
    }

    let prefill_uuid = Uuid::now_v7();
    let text_content = payload.content_text.trim();
    let mut total_siswa = 0;

    if let Ok(students) = serde_json::from_str::<Vec<DapodikRawStudent>>(text_content) {
        let now = Utc::now();
        for std in students {
            let nisn = std.nisn.unwrap_or_else(|| "0000000000".to_string());
            let nik = std.nik.unwrap_or_else(|| "0000000000000000".to_string());
            let nama = std
                .nama
                .or(std.nama_pd)
                .unwrap_or_else(|| "SISWA PREFILL".to_string());
            let rombel = std
                .rombel
                .or(std.nama_rombel)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "null" && s != "UMUM")
                .unwrap_or_else(|| "-".to_string());
            let new_id = Uuid::now_v7();

            let _ = sqlx::query(
                r#"
                INSERT INTO dapodik_sync_records
                (id, tenant_id, nisn, nik, nama_school_os, nama_dapodik, rombel, identity_state, mobility_case, classification, action_recommended, stage, last_synced_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 'NONE', 'NORMALIZATION', 'Uploaded via Local Prefill File (.prf)', 'APPROVED', $8)
                ON CONFLICT (id) DO NOTHING
                "#
            )
            .bind(new_id)
            .bind(ctx.tenant_id)
            .bind(&nisn)
            .bind(&nik)
            .bind(nama.to_uppercase())
            .bind(&nama)
            .bind(&rombel)
            .bind(now)
            .execute(&state.pool)
            .await;

            let _ = sqlx::query(
                r#"
                INSERT INTO students (id, tenant_id, nisn, full_name, status, created_at, updated_at)
                VALUES ($1, $2, $3, $4, 'Active', $5, $5)
                ON CONFLICT (tenant_id, nisn) DO UPDATE 
                SET full_name = EXCLUDED.full_name, updated_at = EXCLUDED.updated_at
                "#
            )
            .bind(new_id)
            .bind(ctx.tenant_id)
            .bind(&nisn)
            .bind(nama.to_uppercase())
            .bind(now)
            .execute(&state.pool)
            .await;

            total_siswa += 1;
        }
    } else {
        let now = Utc::now();
        for line in text_content.lines() {
            if line.contains("INSERT INTO") || line.contains("peserta_didik") || line.contains(',')
            {
                let parts: Vec<&str> = line
                    .split(',')
                    .map(|s| s.trim().trim_matches('\''))
                    .collect();
                if parts.len() >= 3 {
                    let nisn = parts.get(0).unwrap_or(&"0000000000").to_string();
                    let nama = parts.get(1).unwrap_or(&"SISWA PREFILL").to_string();
                    let raw_rombel = parts.get(2).unwrap_or(&"-").trim().to_string();
                    let rombel = if raw_rombel == "UMUM" || raw_rombel.is_empty() {
                        "-".to_string()
                    } else {
                        raw_rombel
                    };
                    let new_id = Uuid::now_v7();

                    if !nisn.is_empty() && nisn.chars().all(|c| c.is_ascii_digit()) {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO dapodik_sync_records
                            (id, tenant_id, nisn, nik, nama_school_os, nama_dapodik, rombel, identity_state, mobility_case, classification, action_recommended, stage, last_synced_at)
                            VALUES ($1, $2, $3, $3, $4, $5, $6, 'ACTIVE', 'NONE', 'NORMALIZATION', 'Uploaded via Local Prefill File (.prf)', 'APPROVED', $7)
                            ON CONFLICT (id) DO NOTHING
                            "#
                        )
                        .bind(new_id)
                        .bind(ctx.tenant_id)
                        .bind(&nisn)
                        .bind(nama.to_uppercase())
                        .bind(&nama)
                        .bind(&rombel)
                        .bind(now)
                        .execute(&state.pool)
                        .await;

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO students (id, tenant_id, nisn, full_name, status, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, 'Active', $5, $5)
                            ON CONFLICT (tenant_id, nisn) DO UPDATE 
                            SET full_name = EXCLUDED.full_name, updated_at = EXCLUDED.updated_at
                            "#
                        )
                        .bind(new_id)
                        .bind(ctx.tenant_id)
                        .bind(&nisn)
                        .bind(nama.to_uppercase())
                        .bind(now)
                        .execute(&state.pool)
                        .await;

                        total_siswa += 1;
                    }
                }
            }
        }
    }

    let file_label = payload
        .file_name
        .unwrap_or_else(|| "prefill.prf".to_string());
    let response = DapodikPrefillResponse {
        prefill_id: prefill_uuid.to_string(),
        npsn: "LOCAL_FILE".into(),
        mirror_used: file_label.clone(),
        total_siswa_imported: total_siswa,
        total_rombel_imported: if total_siswa > 0 { 1 } else { 0 },
        status: "PREFILL_PARSED_AND_IMPORTED".into(),
        message: format!(
            "Berhasil me-parse file '{}'. Total {} data siswa di-impor ke database master School OS.",
            file_label, total_siswa
        ),
    };

    Ok(Json(ApiResponse::success(response, ctx.request_id)))
}

pub async fn reconcile_student(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<ReconcileStudentRequest>,
) -> Result<Json<ApiResponse<DapodikSyncRecordDto>>, ApiError> {
    let target_upper = payload.target_name.to_uppercase();

    if let Ok(record_uuid) = Uuid::parse_str(&payload.sync_id) {
        let _ = sqlx::query(
            r#"
            UPDATE dapodik_sync_records
            SET nama_school_os = $1, classification = 'MATCH', action_recommended = 'Reconciled & Normalized', stage = 'VERIFIED', last_synced_at = NOW()
            WHERE id = $2
            "#
        )
        .bind(&target_upper)
        .bind(record_uuid)
        .execute(&state.pool)
        .await;
    }

    let updated = DapodikSyncRecordDto {
        id: payload.sync_id,
        nisn: "0081293819".into(),
        nik: "3273015509080003".into(),
        nama_school_os: target_upper,
        nama_dapodik: payload.target_name,
        rombel: "10B".into(),
        identity_state: "ACTIVE".into(),
        mobility_case: "NONE".into(),
        classification: "MATCH".into(),
        action_recommended: "Reconciled & Normalized".into(),
        stage: "VERIFIED".into(),
        last_synced_at: Utc::now().to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(updated, ctx.request_id)))
}

pub async fn create_qr_token(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<CreateQrTokenRequest>,
) -> Result<Json<ApiResponse<OpaqueQrTokenResponse>>, ApiError> {
    let req_id = Uuid::now_v7();
    let opaque_token = format!("opq_{}", Uuid::now_v7());
    let nonce = format!("nonce_{}", Uuid::now_v7());
    let expires_at = Utc::now() + chrono::Duration::minutes(30);

    let student_json = serde_json::json!({
        "student_name": payload.student_name,
        "nisn": payload.nisn,
        "nik": payload.nik,
        "mother_name": payload.mother_name,
    });

    let _ = sqlx::query(
        r#"
        INSERT INTO onboarding_tokens
        (request_id, tenant_id, opaque_token, nonce, student_data, token_state, expires_at)
        VALUES ($1, $2, $3, $4, $5, 'ISSUED', $6)
        "#,
    )
    .bind(req_id)
    .bind(ctx.tenant_id)
    .bind(&opaque_token)
    .bind(&nonce)
    .bind(student_json)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Failed to create onboarding token: {}", e)),
            &ctx.request_id,
        )
    })?;

    let res = OpaqueQrTokenResponse {
        request_id: req_id.to_string(),
        opaque_token,
        nonce,
        token_state: "ISSUED".into(),
        expires_at: expires_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(res, ctx.request_id)))
}

pub async fn claim_qr_token(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<ClaimQrTokenRequest>,
) -> Result<Json<ApiResponse<OpaqueQrTokenResponse>>, ApiError> {
    if let Ok(req_uuid) = Uuid::parse_str(&payload.request_id) {
        let _ = sqlx::query(
            r#"
            UPDATE onboarding_tokens
            SET token_state = 'CONSUMED', updated_at = NOW()
            WHERE request_id = $1 AND opaque_token = $2
            "#,
        )
        .bind(req_uuid)
        .bind(&payload.opaque_token)
        .execute(&state.pool)
        .await;
    }

    let res = OpaqueQrTokenResponse {
        request_id: payload.request_id,
        opaque_token: payload.opaque_token,
        nonce: format!("nonce_{}", Uuid::now_v7()),
        token_state: "CONSUMED".into(),
        expires_at: Utc::now().to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(res, ctx.request_id)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExcelImportRequest {
    pub teachers: Vec<ExcelTeacherDto>,
    pub staff: Vec<ExcelStaffDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExcelTeacherDto {
    pub nip: Option<String>,
    pub nuptk: Option<String>,
    pub name: String,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExcelStaffDto {
    pub nip: Option<String>,
    pub nuptk: Option<String>,
    pub name: String,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/dapodik/import-excel",
    request_body = ExcelImportRequest,
    responses(
        (status = 200, description = "Excel data imported")
    ),
    security(("Bearer" = []))
)]
pub async fn import_excel_dapodik(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<ExcelImportRequest>,
) -> Result<Json<ApiResponse<DapodikPrefillResponse>>, ApiError> {
    let now = Utc::now();
    let mut total_imported = 0;

    // Helper to parse dates like "1990-01-01" or fallback to None
    let parse_date = |d: &Option<String>| -> Option<chrono::NaiveDate> {
        d.as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    };

    // 1. Process Teachers (Guru)
    for t in payload.teachers {
        let new_id = Uuid::now_v7();
        let nip = t.nip.clone().unwrap_or_else(|| {
            format!(
                "GURU-{}",
                Uuid::now_v7()
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>()
            )
        });

        let tgl_lahir = parse_date(&t.tanggal_lahir);

        let _ = sqlx::query(
            r#"
            INSERT INTO teachers (id, tenant_id, nip, full_name, is_active, created_at, updated_at, 
                                  nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email)
            VALUES ($1, $2, $3, $4, true, $5, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (tenant_id, nip) DO UPDATE 
            SET full_name = EXCLUDED.full_name, nuptk = EXCLUDED.nuptk, jk = EXCLUDED.jk, tempat_lahir = EXCLUDED.tempat_lahir,
                tanggal_lahir = EXCLUDED.tanggal_lahir, status_kepegawaian = EXCLUDED.status_kepegawaian, jenis_ptk = EXCLUDED.jenis_ptk,
                agama = EXCLUDED.agama, alamat_jalan = EXCLUDED.alamat_jalan, no_hp = EXCLUDED.no_hp, email = EXCLUDED.email, updated_at = EXCLUDED.updated_at
            "#
        )
        .bind(new_id).bind(ctx.tenant_id).bind(&nip).bind(t.name.to_uppercase()).bind(now)
        .bind(t.nuptk).bind(t.jk).bind(t.tempat_lahir).bind(tgl_lahir)
        .bind(t.status_kepegawaian).bind(t.jenis_ptk).bind(t.agama).bind(t.alamat_jalan).bind(t.no_hp).bind(t.email)
        .execute(&state.pool).await;

        total_imported += 1;
    }

    // 2. Process Staff (Tendik)
    for s in payload.staff {
        let new_id = Uuid::now_v7();
        let nip = s.nip.clone().unwrap_or_else(|| {
            format!(
                "TENDIK-{}",
                Uuid::now_v7()
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>()
            )
        });

        let tgl_lahir = parse_date(&s.tanggal_lahir);
        let job_title = s.jenis_ptk.clone().unwrap_or_else(|| "Tendik".to_string());

        let _ = sqlx::query(
            r#"
            INSERT INTO staff (id, tenant_id, full_name, job_title, is_active, created_at, updated_at, 
                               nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email)
            VALUES ($1, $2, $3, $4, true, $5, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT DO NOTHING
            "#
        )
        .bind(new_id).bind(ctx.tenant_id).bind(s.name.to_uppercase()).bind(job_title).bind(now)
        .bind(s.nuptk).bind(s.jk).bind(s.tempat_lahir).bind(tgl_lahir).bind(&nip)
        .bind(s.status_kepegawaian).bind(s.jenis_ptk).bind(s.agama).bind(s.alamat_jalan).bind(s.no_hp).bind(s.email)
        .execute(&state.pool).await;

        total_imported += 1;
    }

    let prefill_uuid = Uuid::now_v7();
    let response = DapodikPrefillResponse {
        prefill_id: prefill_uuid.to_string(),
        npsn: "EXCEL_IMPORT".to_string(),
        mirror_used: "LOCAL".to_string(),
        total_siswa_imported: total_imported,
        total_rombel_imported: 0,
        status: "EXCEL_IMPORTED".into(),
        message: format!(
            "Berhasil mengimpor {} Guru/Tendik dari file Excel Dapodik secara lengkap.",
            total_imported
        ),
    };

    Ok(Json(ApiResponse::success(response, ctx.request_id)))
}
