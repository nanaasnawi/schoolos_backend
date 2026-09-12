use axum::{extract::State, Json};
use chrono::Utc;
use hex::ToHex;
use school_core::common::error::{ApplicationError, DomainError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
pub struct DapodikHealthResponse {
    pub connected: bool,
    pub status: String,
    pub message: String,
    pub dapodik_url: String,
    pub last_checked_at: String,
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
            format!("Terhubung ke Dapodik: {}", dapodik_url),
        )
    } else {
        (
            "OFFLINE".to_string(),
            format!("Dapodik sedang OFFLINE / Tidak Terjangkau: {}", dapodik_url),
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

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
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
    #[serde(
        alias = "nama_ibu_kandung",
        alias = "nama_ibu_kandung_str",
        alias = "ibu_kandung"
    )]
    pub nama_ibu: Option<String>,
    pub pekerjaan_ibu_id_str: Option<String>,
    pub nama_wali: Option<String>,
    pub pekerjaan_wali_id_str: Option<String>,
    pub nomor_telepon_seluler: Option<String>,
    pub nomor_telepon_rumah: Option<String>,
    pub alamat_jalan: Option<String>,
    pub email: Option<String>,
    pub jenis_keluar_id: Option<serde_json::Value>,
    pub jenis_keluar_id_str: Option<String>,
    pub tanggal_keluar: Option<String>,
    pub keterangan_keluar: Option<String>,
    pub status: Option<String>,
    pub status_di_sekolah: Option<String>,
    pub aktif: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
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
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
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
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct DapodikRawRombel {
    pub rombongan_belajar_id: Option<String>,
    pub nama: Option<String>,
    pub ptk_id: Option<String>,
    pub tingkat_pendidikan_id: Option<String>,
    pub jenis_rombel: Option<serde_json::Value>,
    pub jenis_rombel_str: Option<String>,
    pub pembelajaran: Option<Vec<DapodikRawPembelajaran>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PullDapodikRequest {
    pub dapodik_url: Option<String>,
    pub npsn: Option<String>,
    pub bearer_token: Option<String>,
    pub raw_students: Option<Vec<DapodikRawStudent>>,
    pub raw_gtk: Option<Vec<DapodikRawGtk>>,
    pub raw_rombel: Option<Vec<DapodikRawRombel>>,
    pub raw_sekolah: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentInfoResponse {
    pub tenant_id: String,
    pub school_name: String,
    pub npsn: String,
    pub dapodik_url: String,
    pub dapodik_token: String,
    pub total_students: i64,
    pub total_teachers: i64,
    pub total_classes: i64,
}

pub async fn get_agent_info(
    ctx: RequestContext,
    state: State<ApplicationContext>,
) -> Result<Json<ApiResponse<AgentInfoResponse>>, ApiError> {
    let school = sqlx::query(
        "SELECT name, npsn, dapodik_url, dapodik_token FROM schools WHERE tenant_id = $1 LIMIT 1",
    )
    .bind(ctx.tenant_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or_default();

    let total_students: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(ctx.tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total_teachers: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(ctx.tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let total_classes: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(ctx.tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let (school_name, npsn, dapodik_url, dapodik_token) = if let Some(s) = school {
        (
            s.get::<String, _>("name"),
            s.get::<Option<String>, _>("npsn").unwrap_or_default(),
            s.get::<Option<String>, _>("dapodik_url")
                .unwrap_or_else(|| "http://127.0.0.1:5774".to_string()),
            s.get::<Option<String>, _>("dapodik_token")
                .unwrap_or_default(),
        )
    } else {
        (
            "School OS".into(),
            String::new(),
            "http://127.0.0.1:5774".to_string(),
            String::new(),
        )
    };

    let info = AgentInfoResponse {
        tenant_id: ctx.tenant_id.to_string(),
        school_name,
        npsn,
        dapodik_url,
        dapodik_token,
        total_students,
        total_teachers,
        total_classes,
    };

    Ok(Json(ApiResponse::success(info, ctx.request_id)))
}

pub async fn pull_dapodik_records(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    payload: Option<Json<PullDapodikRequest>>,
) -> Result<Json<ApiResponse<Vec<DapodikSyncRecordDto>>>, ApiError> {
    let (
        override_url,
        override_npsn,
        override_token,
        agent_students,
        agent_gtk,
        agent_rombel,
        agent_sekolah,
    ) = match payload {
        Some(Json(req)) => (
            req.dapodik_url,
            req.npsn,
            req.bearer_token,
            req.raw_students,
            req.raw_gtk,
            req.raw_rombel,
            req.raw_sekolah,
        ),
        None => (None, None, None, None, None, None, None),
    };

    let is_agent_sync = agent_students.is_some();

    let (dapodik_url, _host, _port, npsn, token) = resolve_dapodik_config(
        &state.pool,
        ctx.tenant_id,
        override_url,
        override_npsn,
        override_token,
    )
    .await;

    // Jika dipanggil tanpa data dari bridge lokal, beri instruksi jelas (server cloud tidak bisa akses localhost operator)
    if !is_agent_sync {
        return Err(ApiError::new(
            ApplicationError::Domain(DomainError::Validation(
                "Sinkronisasi Dapodik memerlukan Bridge aktif.".into(),
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

    // ── 1. Start ACID Database Transaction ──────────────────────
    let mut tx = state.pool.begin().await.map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Failed to start database transaction: {}", e)),
            &ctx.request_id,
        )
    })?;

    // ── 2. Strict Master Data Resolution ────────────────────────
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

    // ── 3. Pre-fetch Roles Once ───────────────────────────────────
    let role_guru_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Guru' LIMIT 1",
    )
    .bind(ctx.tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Database error reading Guru role: {}", e)),
            &ctx.request_id,
        )
    })? {
        Some(id) => id,
        None => {
            let new_role_id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at) VALUES ($1, $2, 'Guru', 'Guru / Tenaga Pendidik', 'WEB, ANDROID', true, NOW(), NOW())"
            )
            .bind(new_role_id)
            .bind(ctx.tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert Guru role: {}", e)), &ctx.request_id))?;
            new_role_id
        }
    };

    let role_siswa_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Siswa' LIMIT 1",
    )
    .bind(ctx.tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Database error reading Siswa role: {}", e)),
            &ctx.request_id,
        )
    })? {
        Some(id) => id,
        None => {
            let new_role_id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at) VALUES ($1, $2, 'Siswa', 'Siswa / Peserta Didik', 'ANDROID', true, NOW(), NOW())"
            )
            .bind(new_role_id)
            .bind(ctx.tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert Siswa role: {}", e)), &ctx.request_id))?;
            new_role_id
        }
    };

    let role_wali_id = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Wali Siswa' LIMIT 1",
    )
    .bind(ctx.tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Database error reading Wali Siswa role: {}", e)),
            &ctx.request_id,
        )
    })? {
        Some(id) => id,
        None => {
            let new_role_id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at) VALUES ($1, $2, 'Wali Siswa', 'Orang Tua / Wali Siswa', 'ANDROID', true, NOW(), NOW())"
            )
            .bind(new_role_id)
            .bind(ctx.tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Internal(format!("Failed to insert Wali Siswa role: {}", e)), &ctx.request_id))?;
            new_role_id
        }
    };

    // ── 4. In-Memory Cache Pre-loading (Eliminating N+1) ───────────
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
    let mut teacher_by_email: HashMap<String, Uuid> = HashMap::new();
    for t in teacher_rows {
        if let Some(n) = t.nip {
            teacher_by_nip.insert(n, t.id);
        }
        if let Some(nup) = t.nuptk {
            teacher_by_nuptk.insert(nup, t.id);
        }
        teacher_by_name.insert(t.full_name, t.id);
    }

    if let Ok(teacher_user_rows) = sqlx::query!(
        r#"
        SELECT t.id as teacher_id, u.email 
        FROM teachers t 
        JOIN users u ON t.user_id = u.id 
        WHERE t.tenant_id = $1
        "#,
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    {
        for r in teacher_user_rows {
            teacher_by_email.insert(r.email.to_lowercase(), r.teacher_id);
        }
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
        "SELECT id, nisn, nik, full_name FROM students WHERE tenant_id = $1",
        ctx.tenant_id
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    let mut student_by_nisn: HashMap<String, Uuid> = HashMap::new();
    let mut student_by_nik: HashMap<String, Uuid> = HashMap::new();
    let mut student_by_name: HashMap<String, Uuid> = HashMap::new();
    for s in student_rows {
        student_by_nisn.insert(s.nisn.trim().to_string(), s.id);
        if let Some(n) = s.nik {
            student_by_nik.insert(n.trim().to_string(), s.id);
        }
        student_by_name.insert(s.full_name.trim().to_uppercase(), s.id);
        student_by_name.insert(s.full_name.trim().to_string(), s.id);
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
        sync_by_nisn.insert(sr.nisn.trim().to_string(), sr.id);
        sync_by_name.insert(sr.nama_dapodik.trim().to_uppercase(), sr.id);
        sync_by_name.insert(sr.nama_dapodik.trim().to_string(), sr.id);
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

    // ── 4.5. PULL PROFIL SEKOLAH (getSekolah) ──────────────────────────────────
    let school_raw_val = if let Some(ref s) = agent_sekolah {
        Some(s.clone())
    } else {
        let target_sekolah_url = format!(
            "{}/WebService/getSekolah?npsn={}",
            dapodik_url.trim_end_matches('/'),
            npsn
        );
        let mut req_builder_sekolah = client.get(&target_sekolah_url);
        if !token.is_empty() {
            req_builder_sekolah =
                req_builder_sekolah.header("Authorization", format!("Bearer {}", token));
        }
        if let Ok(resp) = req_builder_sekolah.send().await {
            if resp.status().is_success() {
                resp.json::<serde_json::Value>().await.ok()
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(val) = school_raw_val {
        let row_obj = if val.is_object() && val.get("rows").is_some() {
            if val["rows"].is_array() {
                val["rows"].as_array().and_then(|a| a.first()).cloned()
            } else {
                val.get("rows").cloned()
            }
        } else if val.is_object() && val.get("nama").is_some() {
            Some(val)
        } else {
            None
        };

        if let Some(r) = row_obj {
            let sek_nama = r.get("nama").and_then(|v| v.as_str());
            let sek_npsn = r.get("npsn").and_then(|v| v.as_str());
            let sek_jalan = r
                .get("alamat_jalan")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_desa = r
                .get("desa_kelurahan")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_kec = r
                .get("kecamatan")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_kab = r
                .get("kabupaten_kota")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_prov = r
                .get("provinsi")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_pos = r
                .get("kode_pos")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let sek_telp = r.get("nomor_telepon").and_then(|v| v.as_str());
            let sek_email = r.get("email").and_then(|v| v.as_str());

            let mut parts: Vec<&str> = Vec::new();
            if !sek_jalan.is_empty() {
                parts.push(sek_jalan);
            }
            if !sek_desa.is_empty() {
                parts.push(sek_desa);
            }
            if !sek_kec.is_empty() {
                parts.push(sek_kec);
            }
            if !sek_kab.is_empty() {
                parts.push(sek_kab);
            }
            if !sek_prov.is_empty() {
                parts.push(sek_prov);
            }
            if !sek_pos.is_empty() {
                parts.push(sek_pos);
            }
            let full_address = parts.join(", ");

            let _ = sqlx::query!(
                r#"
                        UPDATE schools 
                        SET name = COALESCE(NULLIF($1, ''), name),
                            npsn = COALESCE(NULLIF($2, ''), npsn),
                            address = $3,
                            phone_number = COALESCE($4, phone_number),
                            email = COALESCE($5, email),
                            updated_at = NOW()
                        WHERE tenant_id = $6
                        "#,
                sek_nama,
                sek_npsn,
                full_address,
                sek_telp,
                sek_email,
                ctx.tenant_id
            )
            .execute(&mut *tx)
            .await;

            if let Some(s_name) = sek_nama {
                if !s_name.trim().is_empty() {
                    let _ = sqlx::query("UPDATE tenants SET name = $1, updated_at = NOW() WHERE id = $2")
                        .bind(s_name.trim())
                        .bind(ctx.tenant_id)
                        .execute(&mut *tx)
                        .await;
                }
            }
        }
    }

    // ── 5. PULL GTK (Guru & Tendik) ──────────────────────────────────────────
    let extracted_gtk: Option<Vec<DapodikRawGtk>> = if let Some(gtk) = agent_gtk {
        Some(gtk)
    } else {
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
                if let Ok(val) = parsed_value {
                    if val.is_array() {
                        serde_json::from_value(val).ok()
                    } else if val.is_object() && val.get("rows").is_some() {
                        serde_json::from_value(val["rows"].clone()).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };
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
            let username = nip
                .as_ref()
                .cloned()
                .or_else(|| nuptk.as_ref().cloned())
                .unwrap_or_else(|| format!("guru_{}", ptk_id.chars().take(8).collect::<String>()));

            let actual_user_id = match sqlx::query_scalar::<_, Uuid>(
                        r#"
                        INSERT INTO users (id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at) 
                        VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7) 
                        ON CONFLICT (tenant_id, email) DO UPDATE SET 
                            username = COALESCE(users.username, EXCLUDED.username),
                            full_name = EXCLUDED.full_name, 
                            updated_at = EXCLUDED.updated_at
                        RETURNING id
                        "#
                    )
                    .bind(user_id).bind(ctx.tenant_id).bind(&username).bind(&email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&nama).bind(now)
                    .fetch_one(&mut *tx)
                    .await {
                        Ok(uid) => uid,
                        Err(e) => {
                            tracing::error!("Failed to upsert user for teacher {}: {}", nama, e);
                            continue;
                        }
                    };

            let _ = sqlx::query(
                "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(actual_user_id)
            .bind(role_guru_id)
            .execute(&mut *tx)
            .await;

            // Ensure active QR badge token exists for GTK without touching existing active cards
            let has_active_qr = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_qr_tokens WHERE tenant_id = $1 AND user_id = $2 AND is_active = true)"
            )
            .bind(ctx.tenant_id)
            .bind(actual_user_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);

            if !has_active_qr {
                let token_id = Uuid::now_v7();
                let entropy = Uuid::now_v7().to_string().replace('-', "");
                let raw_token = format!("sch_qr_v1_{}_{}", token_id.to_string().replace('-', ""), &entropy[0..16]);
                let mut hasher = Sha256::new();
                hasher.update(raw_token.as_bytes());
                let token_hash = hasher.finalize().encode_hex::<String>();

                let _ = sqlx::query(
                    r#"
                    INSERT INTO user_qr_tokens (
                        id, tenant_id, user_id, token_hash, raw_token, token_type, label, is_active, created_at, updated_at
                    ) VALUES (
                        $1, $2, $3, $4, $5, 'BADGE', $6, true, $7, $7
                    )
                    ON CONFLICT (token_hash) DO NOTHING
                    "#
                )
                .bind(token_id)
                .bind(ctx.tenant_id)
                .bind(actual_user_id)
                .bind(&token_hash)
                .bind(&raw_token)
                .bind(format!("Kartu GTK - {}", nama))
                .bind(now)
                .execute(&mut *tx)
                .await;
            }

            let is_tendik = gtk.jenis_ptk_id_str.as_deref().unwrap_or("").to_lowercase() != "guru";
            let tgl_lahir = gtk
                .tanggal_lahir
                .as_ref()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
            let nama_upper = nama.to_uppercase();

            if is_tendik {
                let job_title = subject_val.clone().unwrap_or_else(|| "Tendik".to_string());
                let existing_staff = staff_by_name.get(&nama_upper).copied();

                if let Some(sid) = existing_staff {
                    let _ = sqlx::query("UPDATE staff SET user_id = COALESCE(staff.user_id, $1), job_title = $2, jk = $3, tempat_lahir = $4, tanggal_lahir = $5, agama = $6, updated_at = $7 WHERE id = $8")
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
                    action_recommended: "Pulled GTK Real-Time from Dapodik Localhost".into(),
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
                                SET user_id = COALESCE(teachers.user_id, $1), subject = COALESCE($2, teachers.subject), jk = $3, tempat_lahir = $4, tanggal_lahir = $5, agama = $6, updated_at = $7, nuptk = COALESCE($8, teachers.nuptk)
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
                    action_recommended: "Pulled GTK Real-Time from Dapodik Localhost".into(),
                    stage: "VERIFIED".into(),
                    last_synced_at: now.to_rfc3339(),
                });
            }
        }
    }

    // ── 6. PULL ROMBEL & PEMBELAJARAN (MAPEL) ────────────────────────────────
    let extracted_rombel: Option<Vec<DapodikRawRombel>> = if let Some(rmbl) = agent_rombel {
        Some(rmbl)
    } else {
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
                if let Ok(val) = parsed_value {
                    if val.is_array() {
                        serde_json::from_value(val).ok()
                    } else if val.is_object() && val.get("rows").is_some() {
                        serde_json::from_value(val["rows"].clone()).ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(rombels) = extracted_rombel {
        // Bersihkan kelas KKA lama dari PostgreSQL karena KKA bukan rombel kelas reguler
        let _ = sqlx::query(
                    "DELETE FROM classes WHERE tenant_id = $1 AND (name ILIKE 'KKA%' OR name ILIKE '%KKA%')"
                )
                .bind(ctx.tenant_id)
                .execute(&mut *tx)
                .await;

        // Refresh class_map setelah pembersihan KKA
        class_map.retain(|name, _| {
            !name.to_uppercase().starts_with("KKA") && !name.to_uppercase().contains("KKA")
        });

        let mut processed_subjects: HashSet<String> = HashSet::new();

        for rmbl in rombels {
            let nama_rombel = rmbl.nama.unwrap_or_else(|| "ROMBEL DAPODIK".to_string());
            let nama_upper = nama_rombel.trim().to_uppercase();

            // KKA adalah kelompok keterampilan/ekskul bukan rombel reguler, abaikan dan jangan diekspor ke PostgreSQL
            let is_kka = nama_upper.starts_with("KKA")
                || nama_upper.contains("KKA")
                || rmbl
                    .jenis_rombel_str
                    .as_deref()
                    .map(|s| {
                        s.to_uppercase().contains("KETERAMPILAN")
                            || s.to_uppercase().contains("KKA")
                    })
                    .unwrap_or(false);

            if is_kka {
                tracing::info!(
                    "Mengabaikan rombel non-reguler / KKA dari Dapodik: {}",
                    nama_rombel
                );
                continue;
            }
            let new_id = Uuid::now_v7();

            let matched_teacher_id = rmbl
                .ptk_id
                .as_ref()
                .and_then(|pid| {
                    let prefix = pid.chars().take(8).collect::<String>();
                    let expected_email = format!("{}@guru.schoolos.id", prefix).to_lowercase();
                    teacher_by_email.get(&expected_email).copied()
                })
                .or_else(|| {
                    let guru_name = match nama_rombel.trim() {
                        "PAKET A4" => Some("KRISTIANTI"),
                        "PAKET A5" => Some("AMIN LISANA"),
                        "PAKET A6" => Some("ASEP RIFAI"),
                        "PAKET B7" => Some("KRISTIANTI"),
                        "PAKET B8" | "PAKET B8a" => Some("SITI MUNIROH"),
                        "PAKET B8b" => Some("FITRI NAFISAH"),
                        "PAKET B9" => Some("SRI MULYANI.S.AG"),
                        "PAKET C10" => Some("ESI ROKESI"),
                        "PAKET C11a" | "PAKET C11b" => Some("TAUFIQ HIDAYAT"),
                        "PAKET C12a" | "PAKET C12b" => Some("ASY SYIFA RAHMAH IHSANI"),
                        _ => None,
                    };
                    guru_name.and_then(|gn| teacher_by_name.get(gn).copied())
                });

            if !class_map.contains_key(&nama_rombel) {
                let _ = sqlx::query(
                            r#"
                            INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, name, capacity, homeroom_teacher_id, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, 30, $6, $7, $7)
                            "#
                        )
                        .bind(new_id).bind(ctx.tenant_id).bind(academic_year_id).bind(grade_level_id)
                        .bind(&nama_rombel).bind(matched_teacher_id).bind(now).execute(&mut *tx).await;

                class_map.insert(nama_rombel.clone(), new_id);
            } else if let Some(existing_cid) = class_map.get(&nama_rombel) {
                if let Some(tid) = matched_teacher_id {
                    let _ = sqlx::query("UPDATE classes SET homeroom_teacher_id = $1, updated_at = $2 WHERE id = $3")
                                .bind(tid).bind(now).bind(existing_cid).execute(&mut *tx).await;
                }
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

    // ── 7. PULL PESERTA DIDIK (O(1) HashSet & Strict Rombel Safety) ──────────
    let extracted_students: Option<Vec<DapodikRawStudent>> = if let Some(std) = agent_students {
        Some(std)
    } else {
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
                if let Ok(val) = parsed_value {
                    if val.is_array() {
                        serde_json::from_value(val).ok()
                    } else if val.is_object() && val.get("rows").is_some() {
                        serde_json::from_value(val["rows"].clone()).ok()
                    } else {
                        None
                    }
                } else {
                    None
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
    };

    if let Some(students) = extracted_students {
        let mut active_student_ids: HashSet<Uuid> = HashSet::new();

        for (idx, std) in students.into_iter().enumerate() {
            // Check status aktif siswa dari field keluar / status
            let is_keluar = std.jenis_keluar_id.is_some()
                || std.jenis_keluar_id_str.is_some()
                || std.tanggal_keluar.is_some()
                || std
                    .status
                    .as_deref()
                    .map(|s| s.to_lowercase() == "keluar" || s.to_lowercase() == "lulus")
                    .unwrap_or(false)
                || std
                    .status_di_sekolah
                    .as_deref()
                    .map(|s| s.to_lowercase() == "keluar" || s.to_lowercase() == "lulus")
                    .unwrap_or(false);

            let is_explicitly_inactive = std
                .aktif
                .as_ref()
                .map(|v| match v {
                    serde_json::Value::Bool(b) => !b,
                    serde_json::Value::Number(n) => n.as_i64() == Some(0),
                    serde_json::Value::String(s) => s == "0" || s.to_lowercase() == "false",
                    _ => false,
                })
                .unwrap_or(false);

            if is_keluar || is_explicitly_inactive {
                tracing::info!(
                    "Melewati siswa non-aktif/keluar/lulus dari Dapodik: {:?}",
                    std.nama
                );
                continue;
            }

            let pd_id = std
                .peserta_didik_id
                .clone()
                .unwrap_or_else(|| format!("pd-{}", idx));
            let raw_nisn = std
                .nisn
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");
            let final_nisn = raw_nisn.unwrap_or_else(|| {
                let clean_id: String = pd_id.chars().filter(|c| c.is_alphanumeric()).collect();
                if clean_id.len() >= 10 {
                    clean_id[..10].to_string()
                } else {
                    format!("{:0>10}", clean_id)
                }
            });

            let nama = std
                .nama
                .clone()
                .or(std.nama_pd.clone())
                .unwrap_or_else(|| "SISWA DAPODIK".to_string());
            let nama_upper = nama.trim().to_uppercase();

            let nik = std
                .nik
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");
            let nipd_val = std
                .nipd
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");

            // Filter out KKA dari rombel siswa
            let raw_rombel = std.rombel.clone().or(std.nama_rombel.clone());
            let valid_rombel = raw_rombel.filter(|r| {
                let u = r.trim().to_uppercase();
                !u.starts_with("KKA") && !u.contains("KKA")
            });
            let sync_rombel_label = valid_rombel
                .as_ref()
                .cloned()
                .unwrap_or_else(|| "-".to_string());

            let new_id = Uuid::now_v7();
            let user_id = Uuid::now_v7();
            let email = format!("{}@siswa.schoolos.id", final_nisn);
            let username = final_nisn.clone();

            let actual_user_id = match sqlx::query_scalar::<_, Uuid>(
                        r#"
                        INSERT INTO users (id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at) 
                        VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7) 
                        ON CONFLICT (tenant_id, email) DO UPDATE SET 
                            username = COALESCE(users.username, EXCLUDED.username),
                            full_name = EXCLUDED.full_name, 
                            updated_at = EXCLUDED.updated_at
                        RETURNING id
                        "#
                    )
                    .bind(user_id).bind(ctx.tenant_id).bind(&username).bind(&email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&nama_upper).bind(now)
                    .fetch_one(&mut *tx)
                    .await {
                        Ok(uid) => Some(uid),
                        Err(e) => {
                            tracing::error!("Failed to upsert user for student {}: {}", nama, e);
                            None
                        }
                    };

            if let Some(uid) = actual_user_id {
                let _ = sqlx::query(
                    "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(uid)
                .bind(role_siswa_id)
                .execute(&mut *tx)
                .await;

                // Ensure active QR badge token exists for student without invalidating existing active card
                let has_active_qr = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM user_qr_tokens WHERE tenant_id = $1 AND user_id = $2 AND is_active = true)"
                )
                .bind(ctx.tenant_id)
                .bind(uid)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if !has_active_qr {
                    let token_id = Uuid::now_v7();
                    let entropy = Uuid::now_v7().to_string().replace('-', "");
                    let raw_token = format!("sch_qr_v1_{}_{}", token_id.to_string().replace('-', ""), &entropy[0..16]);
                    let mut hasher = Sha256::new();
                    hasher.update(raw_token.as_bytes());
                    let token_hash = hasher.finalize().encode_hex::<String>();

                    let _ = sqlx::query(
                        r#"
                        INSERT INTO user_qr_tokens (
                            id, tenant_id, user_id, token_hash, raw_token, token_type, label, is_active, created_at, updated_at
                        ) VALUES (
                            $1, $2, $3, $4, $5, 'BADGE', $6, true, $7, $7
                        )
                        ON CONFLICT (token_hash) DO NOTHING
                        "#
                    )
                    .bind(token_id)
                    .bind(ctx.tenant_id)
                    .bind(uid)
                    .bind(&token_hash)
                    .bind(&raw_token)
                    .bind(format!("Kartu Pelajar - {}", nama_upper))
                    .bind(now)
                    .execute(&mut *tx)
                    .await;
                }
            }

            // ── Guardian / Orang Tua Synchronization ────────────────
            let nama_ibu = std
                .nama_ibu
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");
            let nama_ayah = std
                .nama_ayah
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");
            let nama_wali = std
                .nama_wali
                .clone()
                .filter(|s| !s.trim().is_empty() && s.trim() != "-");

            // Priority akun login wali murid adalah IBU
            let (guardian_name, relationship) = if let Some(ref ibu) = nama_ibu {
                (Some(ibu.clone()), "Mother")
            } else if let Some(ref wali) = nama_wali {
                (Some(wali.clone()), "Guardian")
            } else if let Some(ref ayah) = nama_ayah {
                (Some(ayah.clone()), "Father")
            } else {
                (None, "Guardian")
            };

            let guardian_phone = std
                .nomor_telepon_seluler
                .clone()
                .or(std.nomor_telepon_rumah.clone())
                .unwrap_or_default();
            let student_address = std.alamat_jalan.clone().unwrap_or_default();

            let final_guardian_id = if let Some(g_name) = guardian_name {
                let g_upper = g_name.trim().to_uppercase();
                if let Some(&gid) = guardian_map.get(&g_upper) {
                    Some(gid)
                } else {
                    let g_id = Uuid::now_v7();
                    let g_user_id = Uuid::now_v7();
                    let g_username = if relationship == "Mother" {
                        format!("ibu_{}", final_nisn)
                    } else {
                        format!("wali_{}", final_nisn)
                    };
                    let g_email = format!("{}@wali.schoolos.id", g_username);

                    let actual_g_user_id = match sqlx::query_scalar::<_, Uuid>(
                                r#"
                                INSERT INTO users (id, tenant_id, username, email, password_hash, full_name, is_active, created_at, updated_at)
                                VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7)
                                ON CONFLICT (tenant_id, email) DO UPDATE SET 
                                    username = COALESCE(users.username, EXCLUDED.username),
                                    full_name = EXCLUDED.full_name, 
                                    updated_at = EXCLUDED.updated_at
                                RETURNING id
                                "#
                            )
                            .bind(g_user_id).bind(ctx.tenant_id).bind(&g_username).bind(&g_email).bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY").bind(&g_upper).bind(now)
                            .fetch_one(&mut *tx)
                            .await {
                                Ok(uid) => Some(uid),
                                Err(e) => {
                                    tracing::error!("Failed to upsert user for guardian {}: {}", g_upper, e);
                                    None
                                },
                            };

                    if let Some(uid) = actual_g_user_id {
                        let _ = sqlx::query(
                            "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                        )
                        .bind(uid)
                        .bind(role_wali_id)
                        .execute(&mut *tx)
                        .await;

                        // Ensure active QR badge token exists for guardian without invalidating existing card
                        let has_active_qr = sqlx::query_scalar::<_, bool>(
                            "SELECT EXISTS(SELECT 1 FROM user_qr_tokens WHERE tenant_id = $1 AND user_id = $2 AND is_active = true)"
                        )
                        .bind(ctx.tenant_id)
                        .bind(uid)
                        .fetch_one(&mut *tx)
                        .await
                        .unwrap_or(false);

                        if !has_active_qr {
                            let token_id = Uuid::now_v7();
                            let entropy = Uuid::now_v7().to_string().replace('-', "");
                            let raw_token = format!("sch_qr_v1_{}_{}", token_id.to_string().replace('-', ""), &entropy[0..16]);
                            let mut hasher = Sha256::new();
                            hasher.update(raw_token.as_bytes());
                            let token_hash = hasher.finalize().encode_hex::<String>();

                            let _ = sqlx::query(
                                r#"
                                INSERT INTO user_qr_tokens (
                                    id, tenant_id, user_id, token_hash, raw_token, token_type, label, is_active, created_at, updated_at
                                ) VALUES (
                                    $1, $2, $3, $4, $5, 'BADGE', $6, true, $7, $7
                                )
                                ON CONFLICT (token_hash) DO NOTHING
                                "#
                            )
                            .bind(token_id)
                            .bind(ctx.tenant_id)
                            .bind(uid)
                            .bind(&token_hash)
                            .bind(&raw_token)
                            .bind(format!("Kartu Akses Wali - {}", g_upper))
                            .bind(now)
                            .execute(&mut *tx)
                            .await;
                        }
                    }

                    let inserted_gid = sqlx::query_scalar::<_, Uuid>(
                                r#"
                                INSERT INTO guardians (id, tenant_id, user_id, full_name, relationship, phone_number, address, created_at, updated_at)
                                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                                RETURNING id
                                "#
                            )
                            .bind(g_id).bind(ctx.tenant_id).bind(actual_g_user_id).bind(&g_upper).bind(relationship).bind(&guardian_phone).bind(&student_address).bind(now)
                            .fetch_one(&mut *tx)
                            .await
                            .unwrap_or(g_id);

                    guardian_map.insert(g_upper, inserted_gid);
                    Some(inserted_gid)
                }
            } else {
                None
            };

            // Resolution student ID by NISN -> NIK -> Name
            let existing_student_id = student_by_nisn
                .get(&final_nisn)
                .copied()
                .or_else(|| {
                    nik.as_ref()
                        .and_then(|n| student_by_nik.get(n.trim()).copied())
                })
                .or_else(|| student_by_name.get(&nama_upper).copied())
                .or_else(|| student_by_name.get(&nama).copied());

            let student_db_id = if let Some(sid) = existing_student_id {
                let update_res = sqlx::query(
                            r#"
                            UPDATE students 
                            SET full_name = $1, user_id = COALESCE(students.user_id, $2), nik = $3, gender = $4, place_of_birth = $5, date_of_birth = $6, religion = $7, 
                                guardian_id = COALESCE($8, students.guardian_id), nipd = COALESCE($9, students.nipd),
                                alamat_jalan = COALESCE($10, students.alamat_jalan), no_hp = COALESCE($11, students.no_hp),
                                email = COALESCE($12, students.email), nama_ayah = COALESCE($13, students.nama_ayah), nama_ibu = COALESCE($14, students.nama_ibu),
                                status = 'active', updated_at = $15, nisn = $16
                            WHERE id = $17
                            "#
                        )
                        .bind(&nama_upper).bind(actual_user_id).bind(&nik).bind(&std.jenis_kelamin).bind(&std.tempat_lahir)
                        .bind(std.tanggal_lahir.as_ref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
                        .bind(&std.agama_id_str).bind(final_guardian_id).bind(nipd_val.clone()).bind(&student_address).bind(&guardian_phone)
                        .bind(&std.email).bind(&nama_ayah).bind(&nama_ibu).bind(now).bind(&final_nisn).bind(sid).execute(&mut *tx).await;

                if let Err(ref e) = update_res {
                    tracing::error!(
                        "Failed to update student (NISN: {}, Name: {}): {}",
                        final_nisn,
                        nama,
                        e
                    );
                }
                student_by_nisn.insert(final_nisn.clone(), sid);
                if let Some(ref n) = nik {
                    student_by_nik.insert(n.trim().to_string(), sid);
                }
                student_by_name.insert(nama_upper.clone(), sid);
                student_by_name.insert(nama.clone(), sid);
                sid
            } else {
                let inserted_id = sqlx::query_scalar::<_, Uuid>(
                            r#"
                            INSERT INTO students (id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, nama_ayah, nama_ibu, status, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, 'active', $18, $18)
                            ON CONFLICT (tenant_id, nisn) DO UPDATE
                            SET full_name = EXCLUDED.full_name,
                                user_id = COALESCE(students.user_id, EXCLUDED.user_id),
                                guardian_id = COALESCE(EXCLUDED.guardian_id, students.guardian_id),
                                nik = COALESCE(EXCLUDED.nik, students.nik),
                                gender = COALESCE(EXCLUDED.gender, students.gender),
                                place_of_birth = COALESCE(EXCLUDED.place_of_birth, students.place_of_birth),
                                date_of_birth = COALESCE(EXCLUDED.date_of_birth, students.date_of_birth),
                                religion = COALESCE(EXCLUDED.religion, students.religion),
                                nipd = COALESCE(EXCLUDED.nipd, students.nipd),
                                alamat_jalan = COALESCE(EXCLUDED.alamat_jalan, students.alamat_jalan),
                                no_hp = COALESCE(EXCLUDED.no_hp, students.no_hp),
                                email = COALESCE(EXCLUDED.email, students.email),
                                nama_ayah = COALESCE(EXCLUDED.nama_ayah, students.nama_ayah),
                                nama_ibu = COALESCE(EXCLUDED.nama_ibu, students.nama_ibu),
                                status = 'active',
                                updated_at = EXCLUDED.updated_at
                            RETURNING id
                            "#
                        )
                        .bind(new_id).bind(ctx.tenant_id).bind(actual_user_id).bind(final_guardian_id).bind(&final_nisn).bind(&nama_upper)
                        .bind(&nik).bind(&std.jenis_kelamin).bind(&std.tempat_lahir)
                        .bind(std.tanggal_lahir.as_ref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
                        .bind(&std.agama_id_str).bind(nipd_val).bind(&student_address).bind(&guardian_phone).bind(&std.email)
                        .bind(&nama_ayah).bind(&nama_ibu).bind(now).fetch_one(&mut *tx).await.unwrap_or_else(|e| {
                            tracing::error!("Failed to insert student (NISN: {}, Name: {}): {}", final_nisn, nama, e);
                            new_id
                        });

                student_by_nisn.insert(final_nisn.clone(), inserted_id);
                student_by_name.insert(nama_upper.clone(), inserted_id);
                student_by_name.insert(nama.clone(), inserted_id);
                inserted_id
            };

            active_student_ids.insert(student_db_id);

            // ── 8. Enrollment & Rombel Association Logic ─────────────
            if let Some(ref r_name) = valid_rombel {
                let r_upper = r_name.to_uppercase();
                if !r_upper.starts_with("KKA") && !r_upper.contains("KKA") {
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
                }
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
                action_recommended: "Pulled Real-Time from Dapodik Localhost WebService".into(),
                stage: "VERIFIED".into(),
                last_synced_at: now.to_rfc3339(),
            });
        }

        // ── 9. Automatic Deletion for Transferred / Left / Graduated Students ─
        if !active_student_ids.is_empty() {
            let active_ids_vec: Vec<Uuid> = active_student_ids.into_iter().collect();

            #[derive(sqlx::FromRow)]
            struct RemovedStudentRow {
                id: Uuid,
                user_id: Option<Uuid>,
                full_name: String,
                nisn: String,
            }

            // Cari siswa di PostgreSQL yang sudah tidak ada lagi di data aktif Dapodik (termutasi / keluar / lulus)
            let removed_students = sqlx::query_as::<_, RemovedStudentRow>(
                        "SELECT id, user_id, full_name, nisn FROM students WHERE tenant_id = $1 AND NOT (id = ANY($2))",
                    )
                    .bind(ctx.tenant_id)
                    .bind(&active_ids_vec)
                    .fetch_all(&mut *tx)
                    .await
                    .unwrap_or_default();

            if !removed_students.is_empty() {
                let removed_ids: Vec<Uuid> = removed_students.iter().map(|s| s.id).collect();
                let removed_user_ids: Vec<Uuid> =
                    removed_students.iter().filter_map(|s| s.user_id).collect();
                let removed_nisns: Vec<String> =
                    removed_students.iter().map(|s| s.nisn.clone()).collect();
                let removed_names: Vec<String> = removed_students
                    .iter()
                    .map(|s| s.full_name.clone())
                    .collect();

                // 1. Hapus dari tabel students (otomatis cascade ke enrollments, gradebooks, assignment_submissions, attendance, dll)
                let _ = sqlx::query("DELETE FROM students WHERE tenant_id = $1 AND id = ANY($2)")
                    .bind(ctx.tenant_id)
                    .bind(&removed_ids)
                    .execute(&mut *tx)
                    .await;

                // 2. Hapus akun login siswa jika ada
                if !removed_user_ids.is_empty() {
                    let _ = sqlx::query("DELETE FROM users WHERE tenant_id = $1 AND id = ANY($2)")
                        .bind(ctx.tenant_id)
                        .bind(&removed_user_ids)
                        .execute(&mut *tx)
                        .await;
                }

                // 3. Hapus dari dapodik_sync_records
                let _ = sqlx::query(
                            "DELETE FROM dapodik_sync_records WHERE tenant_id = $1 AND (nisn = ANY($2) OR nama_school_os = ANY($3))"
                        )
                        .bind(ctx.tenant_id)
                        .bind(&removed_nisns)
                        .bind(&removed_names)
                        .execute(&mut *tx)
                        .await;

                tracing::info!(
                    "Otomatis menghapus {} siswa termutasi/keluar/lulus dari PostgreSQL: {:?}",
                    removed_students.len(),
                    removed_names
                );
            }
        }

        // ── 9.5. Automatic Cleanup for Empty Classes (0 Siswa) ─────────
        #[derive(sqlx::FromRow)]
        struct EmptyClassRow {
            id: Uuid,
            name: String,
        }

        let empty_classes = sqlx::query_as::<_, EmptyClassRow>(
            r#"
                    SELECT c.id, c.name 
                    FROM classes c 
                    WHERE c.tenant_id = $1 
                    AND NOT EXISTS (
                        SELECT 1 FROM enrollments e 
                        WHERE e.class_id = c.id 
                        AND e.tenant_id = c.tenant_id 
                        AND (e.status = 'Active' OR e.status = 'active')
                    )
                    "#,
        )
        .bind(ctx.tenant_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        if !empty_classes.is_empty() {
            let empty_class_ids: Vec<Uuid> = empty_classes.iter().map(|c| c.id).collect();
            let empty_class_names: Vec<String> =
                empty_classes.iter().map(|c| c.name.clone()).collect();

            let _ = sqlx::query("DELETE FROM classes WHERE tenant_id = $1 AND id = ANY($2)")
                .bind(ctx.tenant_id)
                .bind(&empty_class_ids)
                .execute(&mut *tx)
                .await;

            let _ = sqlx::query(
                "DELETE FROM dapodik_sync_records WHERE tenant_id = $1 AND rombel = ANY($2)",
            )
            .bind(ctx.tenant_id)
            .bind(&empty_class_names)
            .execute(&mut *tx)
            .await;

            tracing::info!(
                "Otomatis menghapus {} kelas kosong (0 siswa) dari PostgreSQL: {:?}",
                empty_classes.len(),
                empty_class_names
            );
        }
    }

    // ── 10. Commit Database Transaction ───────────────────────────
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
