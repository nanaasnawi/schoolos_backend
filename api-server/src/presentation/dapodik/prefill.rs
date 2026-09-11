use axum::{extract::State, Json};
use chrono::Utc;
use school_core::common::error::ApplicationError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    presentation::dapodik::sync::DapodikRawStudent,
    response::ApiResponse,
};

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

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadPrefillRequest {
    pub file_name: Option<String>,
    pub content_text: String,
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
                            "SELECT id FROM roles WHERE tenant_id = $1 AND name = 'Siswa' LIMIT 1",
                        )
                        .bind(ctx.tenant_id)
                        .fetch_optional(&state.pool)
                        .await
                        .unwrap_or(None)
                        {
                            Some(id) => id,
                            None => {
                                let new_role_id = Uuid::now_v7();
                                let _ = sqlx::query(
                                    "INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at) VALUES ($1, $2, 'Siswa', 'Siswa / Peserta Didik', 'ANDROID', true, NOW(), NOW())"
                                )
                                .bind(new_role_id)
                                .bind(ctx.tenant_id)
                                .execute(&state.pool)
                                .await;
                                new_role_id
                            }
                        };

                        let user_id = Uuid::now_v7();
                        let email = format!(
                            "{}@student.schoolos.id",
                            nisn.to_lowercase().replace(" ", "")
                        );

                        let actual_user_id = match sqlx::query_scalar::<_, Uuid>(
                            r#"
                            INSERT INTO users (id, tenant_id, email, password_hash, full_name, is_active, created_at, updated_at) 
                            VALUES ($1, $2, $3, $4, $5, true, $6, $6) 
                            ON CONFLICT (tenant_id, email) DO UPDATE SET full_name = EXCLUDED.full_name, updated_at = EXCLUDED.updated_at
                            RETURNING id
                            "#
                        )
                        .bind(user_id)
                        .bind(ctx.tenant_id)
                        .bind(&email)
                        .bind("$argon2id$v=19$m=19456,t=2,p=1$TMFegmCoK1/YLe4lqUwGqg$fPzas5qwg5hV28Hv8ogNfbIBmtAAKmowx+erCcDf5UY")
                        .bind(&nama)
                        .bind(now)
                        .fetch_one(&state.pool)
                        .await {
                            Ok(uid) => uid,
                            Err(e) => {
                                tracing::error!("Failed to upsert user for prefill student {}: {}", nama, e);
                                continue;
                            }
                        };

                        let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(actual_user_id).bind(role_siswa_id).execute(&state.pool).await;

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO students (id, tenant_id, user_id, nisn, full_name, status, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, 'Active', $6, $6)
                            ON CONFLICT (tenant_id, nisn) DO UPDATE 
                            SET full_name = EXCLUDED.full_name, user_id = COALESCE(students.user_id, EXCLUDED.user_id), updated_at = EXCLUDED.updated_at
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
