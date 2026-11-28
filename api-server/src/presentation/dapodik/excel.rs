use axum::{extract::State, Json};
use chrono::Utc;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::prefill::DapodikPrefillResponse;
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

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
    super::controller::require_dapodik_admin(&ctx)?;

    let now = Utc::now();
    let mut total_imported = 0;

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
        let clean_nuptk = s
            .nuptk
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty() && *v != "-")
            .map(str::to_string);
        let clean_nip = s
            .nip
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty() && *v != "-")
            .map(str::to_string);
        let nip = clean_nip.clone().unwrap_or_else(|| {
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
        let name_trimmed = s.name.trim();
        let name_base_lower = name_trimmed
            .to_lowercase()
            .split(',')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        // Match existing active staff by NUPTK, non-synthetic NIP, or normalized name to avoid duplicates
        let existing_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT id
            FROM staff
            WHERE tenant_id = $1
              AND deleted_at IS NULL
              AND (
                ($2::text IS NOT NULL AND TRIM(nuptk) = $2)
                OR ($3::text IS NOT NULL AND nip = $3)
                OR ($4 <> '' AND LOWER(REGEXP_REPLACE(TRIM(SPLIT_PART(full_name, ',', 1)), '\s+', ' ', 'g')) = $4)
              )
            ORDER BY
              CASE WHEN $2::text IS NOT NULL AND TRIM(nuptk) = $2 THEN 0
                   WHEN $3::text IS NOT NULL AND nip = $3 THEN 1
                   ELSE 2 END,
              created_at ASC
            LIMIT 1
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(&clean_nuptk)
        .bind(&clean_nip)
        .bind(&name_base_lower)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();

        if let Some(sid) = existing_id {
            let _ = sqlx::query(
                r#"
                UPDATE staff
                SET nuptk = COALESCE($2, staff.nuptk),
                    jk = COALESCE($3, staff.jk),
                    tempat_lahir = COALESCE($4, staff.tempat_lahir),
                    tanggal_lahir = COALESCE($5, staff.tanggal_lahir),
                    nip = COALESCE($6, staff.nip),
                    status_kepegawaian = COALESCE($7, staff.status_kepegawaian),
                    jenis_ptk = COALESCE($8, staff.jenis_ptk),
                    agama = COALESCE($9, staff.agama),
                    alamat_jalan = COALESCE($10, staff.alamat_jalan),
                    no_hp = COALESCE($11, staff.no_hp),
                    email = COALESCE($12, staff.email),
                    job_title = CASE WHEN staff.job_title = 'Tendik' OR TRIM(staff.job_title) = '' THEN $13 ELSE staff.job_title END,
                    is_active = true,
                    updated_at = $14
                WHERE id = $1
                "#,
            )
            .bind(sid)
            .bind(&clean_nuptk)
            .bind(&s.jk)
            .bind(&s.tempat_lahir)
            .bind(tgl_lahir)
            .bind(&clean_nip)
            .bind(&s.status_kepegawaian)
            .bind(&s.jenis_ptk)
            .bind(&s.agama)
            .bind(&s.alamat_jalan)
            .bind(&s.no_hp)
            .bind(&s.email)
            .bind(&job_title)
            .bind(now)
            .execute(&state.pool)
            .await;
        } else {
            let _ = sqlx::query(
                r#"
                INSERT INTO staff (id, tenant_id, full_name, job_title, is_active, created_at, updated_at, 
                                   nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email)
                VALUES ($1, $2, $3, $4, true, $5, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                ON CONFLICT DO NOTHING
                "#
            )
            .bind(new_id).bind(ctx.tenant_id).bind(s.name.to_uppercase()).bind(job_title).bind(now)
            .bind(&clean_nuptk).bind(s.jk).bind(s.tempat_lahir).bind(tgl_lahir).bind(&nip)
            .bind(s.status_kepegawaian).bind(s.jenis_ptk).bind(s.agama).bind(s.alamat_jalan).bind(s.no_hp).bind(s.email)
            .execute(&state.pool).await;
        }

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
