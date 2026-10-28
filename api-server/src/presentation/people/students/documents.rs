use axum::{
    extract::{Multipart, Path, Query, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use super::dto::document_dto::{
    ClassDossierStat, DocumentItemDto, RekapQuery, RekapRombelDossierResponse,
    StudentDocumentSummaryRow, StudentDossierOverviewResponse, VerifyDocumentPayload,
};
use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error_code::ErrorCode;

#[derive(Clone, Copy)]
pub struct DocSpec {
    pub doc_type: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub is_required: bool,
}

pub fn evaluate_tier(class_name: Option<&str>) -> (&'static str, &'static str) {
    if let Some(c) = class_name {
        let upper = c.to_uppercase();
        if upper.contains("PAKET A") || upper.contains("SD") {
            return ("PAKET_A", "Paket A (Setara SD)");
        }
        if upper.contains("PAKET B") || upper.contains("SMP") {
            return ("PAKET_B", "Paket B (Setara SMP)");
        }
        if upper.contains("PAKET C") || upper.contains("SMA") || upper.contains("SMK") {
            return ("PAKET_C", "Paket C (Setara SMA)");
        }
    }
    ("PAKET_C", "Paket C (Setara SMA)")
}

pub fn get_specs_for_tier(tier: &str) -> Vec<DocSpec> {
    match tier {
        "PAKET_A" => vec![
            DocSpec {
                doc_type: "FORMULIR",
                label: "Formulir Pendaftaran",
                description: "Scan atau foto Formulir Pendaftaran Siswa Baru yang telah diisi lengkap dan ditandatangani",
                is_required: true,
            },
            DocSpec {
                doc_type: "KK",
                label: "Kartu Keluarga (KK)",
                description: "Scan atau foto jelas KK asli dengan barcode / tanda tangan basah Dukcapil",
                is_required: true,
            },
            DocSpec {
                doc_type: "AKTA",
                label: "Akta Kelahiran",
                description: "Scan atau foto jelas Akta Kelahiran resmi dari Dinas Kependudukan dan Catatan Sipil",
                is_required: true,
            },
            DocSpec {
                doc_type: "PAS_FOTO",
                label: "Pas Foto Resmi (3x4)",
                description: "Pas foto formal berseragam rapi dengan latar belakang merah atau biru",
                is_required: false,
            },
            DocSpec {
                doc_type: "KIP",
                label: "KIP / PIP / KKS / PKH",
                description: "Kartu bantuan sosial pemerintah jika mengajukan program beasiswa PIP",
                is_required: false,
            },
        ],
        "PAKET_B" => vec![
            DocSpec {
                doc_type: "FORMULIR",
                label: "Formulir Pendaftaran",
                description: "Scan atau foto Formulir Pendaftaran Siswa Baru yang telah diisi lengkap dan ditandatangani",
                is_required: true,
            },
            DocSpec {
                doc_type: "KK",
                label: "Kartu Keluarga (KK)",
                description: "Scan atau foto jelas KK asli dengan barcode / tanda tangan basah Dukcapil",
                is_required: true,
            },
            DocSpec {
                doc_type: "IJAZAH_SD",
                label: "Ijazah SD / Sederajat",
                description: "Scan atau foto asli Ijazah SD/MI/Paket A yang memuat nomor seri resmi",
                is_required: true,
            },
            DocSpec {
                doc_type: "KTP",
                label: "KTP (Warga Belajar Dewasa)",
                description: "KTP Elektronik siswa (diperlukan jika siswa berusia 17 tahun ke atas)",
                is_required: false,
            },
            DocSpec {
                doc_type: "PAS_FOTO",
                label: "Pas Foto Resmi (3x4)",
                description: "Pas foto formal berseragam rapi dengan latar belakang merah atau biru",
                is_required: false,
            },
            DocSpec {
                doc_type: "KIP",
                label: "KIP / PIP / KKS / PKH",
                description: "Kartu bantuan sosial pemerintah jika mengajukan program beasiswa PIP",
                is_required: false,
            },
        ],
        _ => vec![
            // PAKET_C
            DocSpec {
                doc_type: "FORMULIR",
                label: "Formulir Pendaftaran",
                description: "Scan atau foto Formulir Pendaftaran Siswa Baru yang telah diisi lengkap dan ditandatangani",
                is_required: true,
            },
            DocSpec {
                doc_type: "KK",
                label: "Kartu Keluarga (KK)",
                description: "Scan atau foto jelas KK asli dengan barcode / tanda tangan basah Dukcapil",
                is_required: true,
            },
            DocSpec {
                doc_type: "IJAZAH_SMP",
                label: "Ijazah SMP / Sederajat",
                description: "Scan atau foto asli Ijazah SMP/MTs/Paket B yang memuat nomor seri resmi",
                is_required: true,
            },
            DocSpec {
                doc_type: "KTP",
                label: "KTP (Warga Belajar Dewasa)",
                description: "KTP Elektronik siswa (diperlukan jika siswa berusia 17 tahun ke atas)",
                is_required: false,
            },
            DocSpec {
                doc_type: "PAS_FOTO",
                label: "Pas Foto Resmi (3x4)",
                description: "Pas foto formal berseragam rapi dengan latar belakang merah atau biru",
                is_required: false,
            },
            DocSpec {
                doc_type: "KIP",
                label: "KIP / PIP / KKS / PKH",
                description: "Kartu bantuan sosial pemerintah jika mengajukan program beasiswa PIP",
                is_required: false,
            },
        ],
    }
}

pub async fn build_student_dossier_response(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    student_id: Uuid,
) -> Result<StudentDossierOverviewResponse, sqlx::Error> {
    // 1. Fetch student info and enrolled class
    let student_row = sqlx::query(
        r#"
        SELECT 
            s.id, s.nisn, s.full_name,
            c.name as class_name
        FROM students s
        LEFT JOIN LATERAL (
            SELECT c.name FROM enrollments e
            JOIN classes c ON c.id = e.class_id
            WHERE e.student_id = s.id AND (e.status = 'active' OR e.status = 'Active')
            ORDER BY e.enrolled_at DESC
            LIMIT 1
        ) c ON true
        WHERE s.id = $1 AND s.tenant_id = $2
        LIMIT 1
        "#,
    )
    .bind(student_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let full_name: String = student_row.get("full_name");
    let nisn: String = student_row.get("nisn");
    let class_name: Option<String> = student_row.get("class_name");

    let (tier_code, tier_label) = evaluate_tier(class_name.as_deref());
    let specs = get_specs_for_tier(tier_code);

    // 2. Fetch existing uploaded documents
    let doc_rows = sqlx::query(
        r#"
        SELECT 
            id, document_type, file_url, file_name, file_size_bytes, mime_type,
            verification_status, verified_at, rejection_reason, notes, updated_at
        FROM student_documents
        WHERE student_id = $1 AND tenant_id = $2
        "#,
    )
    .bind(student_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    use std::collections::HashMap;
    let mut uploaded_map = HashMap::new();
    for row in doc_rows {
        let dtype: String = row.get("document_type");
        uploaded_map.insert(dtype, row);
    }

    let mut items: Vec<DocumentItemDto> = Vec::new();
    let mut required_count = 0;
    let mut verified_count = 0;
    let mut pending_count = 0;
    let mut rejected_count = 0;

    for spec in specs {
        if spec.is_required {
            required_count += 1;
        }

        if let Some(r) = uploaded_map.get(spec.doc_type) {
            let status: String = r.get("verification_status");
            if status == "VERIFIED" {
                if spec.is_required {
                    verified_count += 1;
                }
            } else if status == "PENDING" {
                pending_count += 1;
            } else if status == "REJECTED" {
                rejected_count += 1;
            }

            let doc_id: Uuid = r.get("id");
            let updated_ts: i64 = r.try_get::<chrono::DateTime<Utc>, _>("updated_at")
                .map(|t| t.timestamp())
                .unwrap_or_else(|_| Utc::now().timestamp());
            let file_url = Some(format!("/api/v1/auth/documents/{doc_id}/file?v={updated_ts}"));
            items.push(DocumentItemDto {
                id: Some(doc_id),
                document_type: spec.doc_type.to_string(),
                label: spec.label.to_string(),
                description: spec.description.to_string(),
                is_required: spec.is_required,
                status,
                file_url,
                file_name: Some(r.get("file_name")),
                file_size_bytes: Some(r.get("file_size_bytes")),
                mime_type: Some(r.get("mime_type")),
                verified_at: r.get("verified_at"),
                rejection_reason: r.get("rejection_reason"),
                notes: r.get("notes"),
                updated_at: Some(r.get("updated_at")),
            });
        } else {
            items.push(DocumentItemDto {
                id: None,
                document_type: spec.doc_type.to_string(),
                label: spec.label.to_string(),
                description: spec.description.to_string(),
                is_required: spec.is_required,
                status: "UNUPLOADED".to_string(),
                file_url: None,
                file_name: None,
                file_size_bytes: None,
                mime_type: None,
                verified_at: None,
                rejection_reason: None,
                notes: None,
                updated_at: None,
            });
        }
    }

    let is_complete = required_count > 0 && verified_count == required_count;
    let completeness_percentage = if required_count > 0 {
        ((verified_count as f64 / required_count as f64) * 1000.0).round() / 10.0
    } else {
        100.0
    };

    Ok(StudentDossierOverviewResponse {
        student_id,
        nisn,
        full_name,
        class_name,
        program_tier: tier_code.to_string(),
        program_tier_label: tier_label.to_string(),
        is_complete,
        completeness_percentage,
        required_count,
        verified_count,
        pending_count,
        rejected_count,
        documents: items,
    })
}

/// GET /api/v1/students/:id/documents
pub async fn get_student_documents(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
) -> Result<Json<ApiResponse<StudentDossierOverviewResponse>>, ApiError> {
    let tenant_id = req_ctx.tenant_id;
    let dossier = build_student_dossier_response(&ctx.pool, tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(dossier, req_ctx.correlation_id.clone())))
}

/// GET /api/v1/students/me/documents
pub async fn get_my_documents(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<StudentDossierOverviewResponse>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidToken,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    // Find student ID linked to this user
    let student_row = sqlx::query(
        "SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id: Uuid = match student_row {
        Some(r) => r.get("id"),
        None => {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    ErrorCode::StudentNotFound,
                    "Data siswa tidak ditemukan untuk akun ini".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let dossier = build_student_dossier_response(&ctx.pool, req_ctx.tenant_id, student_id)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    Ok(Json(ApiResponse::success(dossier, req_ctx.correlation_id.clone())))
}

/// GET /api/v1/auth/documents/:id/file
/// Stream student document file directly from PostgreSQL BYTEA or local disk cache
pub async fn stream_student_document_file(
    State(ctx): State<ApplicationContext>,
    Path(doc_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    let row = sqlx::query(
        "SELECT file_data, file_url, file_name, mime_type FROM student_documents WHERE id = $1",
    )
    .bind(doc_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &Uuid::new_v4().to_string(),
        )
    })?;

    if let Some(r) = row {
        let file_data: Option<Vec<u8>> = r.try_get("file_data").ok().flatten();
        let mime_type: String = r.try_get("mime_type").unwrap_or_else(|_| "application/pdf".to_string());
        let file_name: String = r.try_get("file_name").unwrap_or_else(|_| "dokumen".to_string());

        if let Some(bytes) = file_data {
            if !bytes.is_empty() {
                let resp = axum::response::Response::builder()
                    .status(axum::http::StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, mime_type)
                    .header(
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{file_name}\""),
                    )
                    .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                    .header(axum::http::header::PRAGMA, "no-cache")
                    .header(axum::http::header::EXPIRES, "0")
                    .body(axum::body::Body::from(bytes))
                    .map_err(|e| {
                        ApiError::new(
                            school_core::common::error::ApplicationError::Internal(e.to_string()),
                            &Uuid::new_v4().to_string(),
                        )
                    })?;
                return Ok(resp);
            }
        }

        // Fallback: check disk
        if let Ok(furl) = r.try_get::<String, _>("file_url") {
            let relative_path = furl.trim_start_matches('/');
            let disk_path = std::path::PathBuf::from(relative_path);
            if let Ok(bytes) = tokio::fs::read(&disk_path).await {
                let resp = axum::response::Response::builder()
                    .status(axum::http::StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, mime_type)
                    .header(
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{file_name}\""),
                    )
                    .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                    .header(axum::http::header::PRAGMA, "no-cache")
                    .header(axum::http::header::EXPIRES, "0")
                    .body(axum::body::Body::from(bytes))
                    .map_err(|e| {
                        ApiError::new(
                            school_core::common::error::ApplicationError::Internal(e.to_string()),
                            &Uuid::new_v4().to_string(),
                        )
                    })?;
                return Ok(resp);
            }
        }
    }

    Err(ApiError::new(
        school_core::common::error::ApplicationError::NotFound(
            school_core::common::error_code::ErrorCode::StudentNotFound,
            "Berkas dokumen tidak ditemukan di server".to_string(),
        ),
        &Uuid::new_v4().to_string(),
    ))
}

/// Helper to save uploaded document file and update database
pub async fn process_document_upload(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    student_id: Uuid,
    mut multipart: Multipart,
    request_id: &str,
) -> Result<DocumentItemDto, ApiError> {
    let uploads_dir = std::path::PathBuf::from("uploads").join("documents");
    tokio::fs::create_dir_all(&uploads_dir).await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(format!(
                "Failed to create uploads directory: {e}"
            )),
            request_id,
        )
    })?;

    let mut doc_type: Option<String> = None;
    let mut notes: Option<String> = None;
    let mut file_url: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_size: i64 = 0;
    let mut mime_type: Option<String> = None;
    let mut file_bytes_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(format!(
                "Multipart error: {e}"
            )),
            request_id,
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "document_type" {
            doc_type = Some(field.text().await.unwrap_or_default().trim().to_uppercase());
        } else if name == "notes" {
            notes = Some(field.text().await.unwrap_or_default().trim().to_string());
        } else if name == "file" {
            let original_name = field.file_name().unwrap_or("dokumen").to_string();
            let ext = std::path::Path::new(&original_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_lowercase();

            let allowed = ["jpg", "jpeg", "png", "webp", "pdf"];
            if !allowed.contains(&ext.as_str()) {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(format!(
                            "Format file .{ext} tidak didukung. Harap gunakan format JPG, PNG, WEBP, atau PDF."
                        )),
                    ),
                    request_id,
                ));
            }

            let resolved_mime = match ext.as_str() {
                "pdf" => "application/pdf",
                "png" => "image/png",
                "webp" => "image/webp",
                _ => "image/jpeg",
            };
            mime_type = Some(resolved_mime.to_string());

            let bytes = field.bytes().await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(format!(
                        "Gagal membaca byte file: {e}"
                    )),
                    request_id,
                )
            })?;

            if bytes.len() > 15 * 1024 * 1024 {
                return Err(ApiError::new(
                    school_core::common::error::ApplicationError::Domain(
                        school_core::common::error::DomainError::Validation(
                            "Ukuran berkas melebihi batas maksimal 15 MB".to_string(),
                        ),
                    ),
                    request_id,
                ));
            }

            file_size = bytes.len() as i64;
            file_name = Some(original_name);

            let unique_file = format!("{}_{}_{}.{}", tenant_id, student_id, Uuid::new_v4(), ext);
            let target_path = uploads_dir.join(&unique_file);

            tokio::fs::write(&target_path, &bytes).await.map_err(|e| {
                ApiError::new(
                    school_core::common::error::ApplicationError::Internal(format!(
                        "Gagal menyimpan file ke disk: {e}"
                    )),
                    request_id,
                )
            })?;

            file_bytes_data = Some(bytes.to_vec());
            file_url = Some(format!("/uploads/documents/{unique_file}"));
        }
    }

    let dtype = doc_type.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Parameter document_type wajib disertakan (contoh: KK, AKTA, IJAZAH_SD, IJAZAH_SMP, KTP, PAS_FOTO, KIP)".to_string(),
                ),
            ),
            request_id,
        )
    })?;

    let furl = file_url.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Berkas file belum diunggah".to_string(),
                ),
            ),
            request_id,
        )
    })?;

    let fname = file_name.unwrap_or_else(|| format!("{dtype}.jpg"));
    let fmime = mime_type.unwrap_or_else(|| "image/jpeg".to_string());

    // Insert or update on conflict (tenant_id, student_id, document_type) with file_data BYTEA persistence
    let saved_row = sqlx::query(
        r#"
        INSERT INTO student_documents (
            tenant_id, student_id, document_type, file_url, file_name, file_size_bytes,
            mime_type, file_data, verification_status, rejection_reason, notes, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, 'PENDING', NULL, $9, NOW()
        )
        ON CONFLICT (tenant_id, student_id, document_type) DO UPDATE SET
            file_url = EXCLUDED.file_url,
            file_name = EXCLUDED.file_name,
            file_size_bytes = EXCLUDED.file_size_bytes,
            mime_type = EXCLUDED.mime_type,
            file_data = EXCLUDED.file_data,
            verification_status = 'PENDING',
            rejection_reason = NULL,
            verified_by = NULL,
            verified_at = NULL,
            notes = COALESCE(EXCLUDED.notes, student_documents.notes),
            updated_at = NOW()
        RETURNING id, document_type, file_url, file_name, file_size_bytes, mime_type, verification_status, rejection_reason, notes, updated_at
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .bind(&dtype)
    .bind(&furl)
    .bind(&fname)
    .bind(file_size)
    .bind(&fmime)
    .bind(file_bytes_data)
    .bind(&notes)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            request_id,
        )
    })?;

    let saved_id: Uuid = saved_row.get("id");
    let stream_url = format!("/api/v1/auth/documents/{saved_id}/file");

    Ok(DocumentItemDto {
        id: Some(saved_id),
        document_type: dtype.clone(),
        label: dtype.clone(),
        description: String::new(),
        is_required: true,
        status: saved_row.get("verification_status"),
        file_url: Some(stream_url),
        file_name: Some(fname),
        file_size_bytes: Some(file_size),
        mime_type: Some(fmime),
        verified_at: None,
        rejection_reason: None,
        notes,
        updated_at: Some(saved_row.get("updated_at")),
    })
}

/// POST /api/v1/students/:id/documents
pub async fn upload_student_document(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(student_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<ApiResponse<DocumentItemDto>>, ApiError> {
    let result = process_document_upload(
        &ctx.pool,
        req_ctx.tenant_id,
        student_id,
        multipart,
        &req_ctx.request_id,
    )
    .await?;

    Ok(Json(ApiResponse::success(result, req_ctx.correlation_id.clone())))
}

/// POST /api/v1/students/me/documents
pub async fn upload_my_document(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    multipart: Multipart,
) -> Result<Json<ApiResponse<DocumentItemDto>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidToken,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_row = sqlx::query(
        "SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(actor_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id: Uuid = match student_row {
        Some(r) => r.get("id"),
        None => {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    ErrorCode::StudentNotFound,
                    "Data siswa tidak ditemukan untuk akun Anda".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let result = process_document_upload(
        &ctx.pool,
        req_ctx.tenant_id,
        student_id,
        multipart,
        &req_ctx.request_id,
    )
    .await?;

    Ok(Json(ApiResponse::success(result, req_ctx.correlation_id.clone())))
}

/// PATCH /api/v1/students/documents/:doc_id/verify
pub async fn verify_student_document(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(doc_id): Path<Uuid>,
    Json(payload): Json<VerifyDocumentPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let actor = req_ctx.actor.as_ref().ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidToken,
                "Authentication required".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;
    let actor_id = actor.id;
    let is_verifier_role = actor.roles.iter().any(|r| {
        let lower = r.trim().to_lowercase();
        lower.contains("admin")
            || lower.contains("operator")
            || lower.contains("kepala")
            || lower.contains("staff")
            || lower.contains("tata usaha")
            || lower == "tu"
    });
    if !is_verifier_role {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Forbidden(
                school_core::common::error_code::ErrorCode::AuthInsufficientPermissions,
                "Hanya Operator Sekolah, TU, Kepala Sekolah, atau Administrator yang dapat memverifikasi dokumen siswa (ADR-0020)".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let new_status = payload.status.trim().to_uppercase();
    if new_status != "VERIFIED" && new_status != "REJECTED" {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Status verifikasi harus 'VERIFIED' atau 'REJECTED'".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    if new_status == "REJECTED" && payload.rejection_reason.as_deref().unwrap_or("").trim().is_empty() {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Alasan penolakan (rejection_reason) wajib diisi saat menolak dokumen".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let updated = sqlx::query(
        r#"
        UPDATE student_documents
        SET 
            verification_status = $1,
            verified_by = $2,
            verified_at = NOW(),
            rejection_reason = $3,
            notes = COALESCE($4, notes),
            updated_at = NOW()
        WHERE id = $5 AND tenant_id = $6
        RETURNING id, student_id, document_type, verification_status
        "#,
    )
    .bind(&new_status)
    .bind(actor_id)
    .bind(&payload.rejection_reason)
    .bind(&payload.notes)
    .bind(doc_id)
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let row = updated.ok_or_else(|| {
        ApiError::new(
            school_core::common::error::ApplicationError::NotFound(
                ErrorCode::StudentNotFound,
                "Dokumen tidak ditemukan".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id: Uuid = row.get("student_id");
    let doc_type: String = row.get("document_type");

    // Send push / in-app notification to student if rejected
    if new_status == "REJECTED" {
        let student_user = sqlx::query("SELECT user_id, full_name FROM students WHERE id = $1")
            .bind(student_id)
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

        if let Some(s_user) = student_user {
            let user_id: Option<Uuid> = s_user.get("user_id");
            if let Some(uid) = user_id {
                let reason = payload.rejection_reason.as_deref().unwrap_or("Perlu perbaikan");
                let notif_title = format!("⚠️ Berkas {doc_type} Perlu Perbaikan");
                let notif_body = format!("Mohon unggah ulang berkas {doc_type}. Catatan TU: {reason}");

                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (
                        id, tenant_id, user_id, title, body, notification_type,
                        channel, reference_type, reference_id, is_read, is_urgent,
                        priority, scheduled_at, created_at
                    ) VALUES (
                        gen_random_uuid(), $1, $2, $3, $4, 'DOSSIER_REVISION',
                        'in_app', 'student_document', $5, false, true,
                        'HIGH', NOW(), NOW()
                    )
                    "#,
                )
                .bind(req_ctx.tenant_id)
                .bind(uid)
                .bind(&notif_title)
                .bind(&notif_body)
                .bind(doc_id)
                .execute(&ctx.pool)
                .await;
            }
        }
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "id": doc_id,
            "student_id": student_id,
            "document_type": doc_type,
            "verification_status": new_status,
            "verified_at": Utc::now()
        }),
        req_ctx.correlation_id.clone(),
    )))
}

/// GET /api/v1/students/documents/rekap-rombel
pub async fn get_rekap_rombel(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<RekapQuery>,
) -> Result<Json<ApiResponse<RekapRombelDossierResponse>>, ApiError> {
    let tenant_id = req_ctx.tenant_id;

    // 1. Fetch all active classes in tenant
    let classes_rows = sqlx::query(
        "SELECT id, name FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY name ASC",
    )
    .bind(tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    // 2. Fetch all active students with enrolled class
    let students_query = r#"
        SELECT 
            s.id as student_id, s.nisn, s.full_name,
            c.id as class_id,
            COALESCE(c.name, '-') as class_name
        FROM students s
        LEFT JOIN LATERAL (
            SELECT c.id, c.name FROM enrollments e
            JOIN classes c ON c.id = e.class_id
            WHERE e.student_id = s.id AND (e.status = 'active' OR e.status = 'Active')
            ORDER BY e.enrolled_at DESC
            LIMIT 1
        ) c ON true
        WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
          AND (s.status IN ('Active', 'active') OR (s.status IS NULL AND s.is_active = true))
          AND (s.status NOT IN ('transferred', 'Transferred', 'mutasi', 'Mutasi', 'keluar', 'Keluar', 'dropped_out', 'graduated', 'Graduated'))
          AND s.is_active IS NOT FALSE
          AND ($2::uuid IS NULL OR c.id = $2)
          AND ($3::text IS NULL OR s.full_name ILIKE ('%' || $3 || '%') OR s.nisn ILIKE ('%' || $3 || '%'))
        ORDER BY c.name ASC NULLS LAST, s.full_name ASC
    "#;

    let students_rows = sqlx::query(students_query)
        .bind(tenant_id)
        .bind(query.class_id)
        .bind(&query.search)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

    // 3. Fetch all document statuses for these students
    let docs_rows = sqlx::query(
        r#"
        SELECT student_id, document_type, verification_status
        FROM student_documents
        WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    use std::collections::HashMap;
    let mut student_docs_map: HashMap<Uuid, HashMap<String, String>> = HashMap::new();
    for d in docs_rows {
        let sid: Uuid = d.get("student_id");
        let dtype: String = d.get("document_type");
        let vstatus: String = d.get("verification_status");
        student_docs_map
            .entry(sid)
            .or_insert_with(HashMap::new)
            .insert(dtype, vstatus);
    }

    let mut students_list = Vec::new();
    let mut total_students = 0i64;
    let mut complete_students = 0i64;
    let mut incomplete_students = 0i64;

    let mut class_stats_map: HashMap<String, (Option<Uuid>, i64, i64)> = HashMap::new();
    for cr in &classes_rows {
        let cname: String = cr.get("name");
        let cid: Uuid = cr.get("id");
        class_stats_map.insert(cname, (Some(cid), 0, 0));
    }

    for row in students_rows {
        let sid: Uuid = row.get("student_id");
        let nisn: String = row.get("nisn");
        let full_name: String = row.get("full_name");
        let class_id: Option<Uuid> = row.get("class_id");
        let class_name: String = row.get("class_name");

        let (tier_code, _) = evaluate_tier(Some(&class_name));
        if let Some(ref pt) = query.program_tier {
            if !pt.is_empty() && pt.to_uppercase() != tier_code {
                continue;
            }
        }

        let empty_map = HashMap::new();
        let s_docs = student_docs_map.get(&sid).unwrap_or(&empty_map);

        let formulir_status = s_docs.get("FORMULIR").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());
        let kk_status = s_docs.get("KK").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());
        let akta_status = s_docs.get("AKTA").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());
        let ijazah_status = match tier_code {
            "PAKET_A" => "-".to_string(),
            "PAKET_B" => s_docs.get("IJAZAH_SD").cloned().unwrap_or_else(|| "UNUPLOADED".to_string()),
            _ => s_docs.get("IJAZAH_SMP").cloned().unwrap_or_else(|| "UNUPLOADED".to_string()),
        };
        let ktp_status = s_docs.get("KTP").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());
        let foto_status = s_docs.get("PAS_FOTO").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());
        let kip_status = s_docs.get("KIP").cloned().unwrap_or_else(|| "UNUPLOADED".to_string());

        // Check required completeness according to PKBM tier (FORMULIR is required across all tiers):
        // Paket A: FORMULIR, KK & AKTA (3 required)
        // Paket B: FORMULIR, KK & IJAZAH_SD (3 required)
        // Paket C: FORMULIR, KK & IJAZAH_SMP (3 required)
        let is_complete = match tier_code {
            "PAKET_A" => formulir_status == "VERIFIED" && kk_status == "VERIFIED" && akta_status == "VERIFIED",
            "PAKET_B" => formulir_status == "VERIFIED" && kk_status == "VERIFIED" && ijazah_status == "VERIFIED",
            _ => formulir_status == "VERIFIED" && kk_status == "VERIFIED" && ijazah_status == "VERIFIED",
        };

        let verified_req_count = match tier_code {
            "PAKET_A" => (if formulir_status == "VERIFIED" { 1 } else { 0 }) + (if kk_status == "VERIFIED" { 1 } else { 0 }) + (if akta_status == "VERIFIED" { 1 } else { 0 }),
            _ => (if formulir_status == "VERIFIED" { 1 } else { 0 }) + (if kk_status == "VERIFIED" { 1 } else { 0 }) + (if ijazah_status == "VERIFIED" { 1 } else { 0 }),
        };
        let percentage = ((verified_req_count as f64 / 3.0) * 1000.0).round() / 10.0;

        if let Some(ref sf) = query.status_filter {
            if sf == "COMPLETE" && !is_complete {
                continue;
            }
            if sf == "INCOMPLETE" && is_complete {
                continue;
            }
        }

        total_students += 1;
        if is_complete {
            complete_students += 1;
        } else {
            incomplete_students += 1;
        }

        // Update class stats
        if let Some(entry) = class_stats_map.get_mut(&class_name) {
            entry.1 += 1; // total in class
            if is_complete {
                entry.2 += 1; // complete in class
            }
        } else {
            class_stats_map.insert(
                class_name.clone(),
                (class_id, 1, if is_complete { 1 } else { 0 }),
            );
        }

        students_list.push(StudentDocumentSummaryRow {
            student_id: sid,
            nisn,
            full_name,
            class_id,
            class_name: Some(class_name),
            program_tier: tier_code.to_string(),
            is_complete,
            completeness_percentage: percentage,
            formulir_status,
            kk_status,
            akta_status,
            ijazah_status,
            ktp_status,
            foto_status,
            kip_status,
        });
    }

    let mut classes_summary = Vec::new();
    for (cname, (cid, tot, comp)) in class_stats_map {
        if tot > 0 {
            let pct = ((comp as f64 / tot as f64) * 1000.0).round() / 10.0;
            classes_summary.push(ClassDossierStat {
                class_id: cid,
                class_name: cname,
                total: tot,
                complete: comp,
                incomplete: tot - comp,
                percentage: pct,
            });
        }
    }
    classes_summary.sort_by(|a, b| a.class_name.cmp(&b.class_name));

    let completeness_percentage = if total_students > 0 {
        ((complete_students as f64 / total_students as f64) * 1000.0).round() / 10.0
    } else {
        100.0
    };

    Ok(Json(ApiResponse::success(
        RekapRombelDossierResponse {
            total_students,
            complete_students,
            incomplete_students,
            completeness_percentage,
            classes_summary,
            students: students_list,
        },
        req_ctx.correlation_id.clone(),
    )))
}

#[derive(Deserialize)]
pub struct BroadcastReminderPayload {
    pub class_id: Option<Uuid>,
    pub message: Option<String>,
}

/// POST /api/v1/students/documents/broadcast-reminder
pub async fn broadcast_document_reminder(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<BroadcastReminderPayload>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let tenant_id = req_ctx.tenant_id;

    // Find all students in class (or tenant) whose documents are not verified
    let students = sqlx::query(
        r#"
        SELECT DISTINCT s.id, s.user_id, s.full_name
        FROM students s
        LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'active' OR e.status = 'Active')
        WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
          AND (s.status IN ('Active', 'active') OR (s.status IS NULL AND s.is_active = true))
          AND (s.status NOT IN ('transferred', 'Transferred', 'mutasi', 'Mutasi', 'keluar', 'Keluar', 'dropped_out', 'graduated', 'Graduated'))
          AND s.is_active IS NOT FALSE
          AND ($2::uuid IS NULL OR e.class_id = $2)
          AND s.user_id IS NOT NULL
        "#,
    )
    .bind(tenant_id)
    .bind(payload.class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let default_msg = "Pemberitahuan: Mohon segera melengkapi berkas persyaratan siswa (KK, Akta / Ijazah) melalui aplikasi SchoolOS untuk keperluan administrasi dan sinkronisasi Dapodik.".to_string();
    let body_text = payload.message.unwrap_or(default_msg);

    let mut sent_count = 0;
    for s in students {
        let uid: Option<Uuid> = s.get("user_id");
        if let Some(user_id) = uid {
            let _ = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type,
                    channel, reference_type, is_read, is_urgent, priority, scheduled_at, created_at
                ) VALUES (
                    gen_random_uuid(), $1, $2, '📋 Pengingat Kelengkapan Berkas Siswa',
                    $3, 'DOSSIER_REMINDER', 'in_app', 'student_dossier', false, true,
                    'HIGH', NOW(), NOW()
                )
                "#,
            )
            .bind(tenant_id)
            .bind(user_id)
            .bind(&body_text)
            .execute(&ctx.pool)
            .await;

            sent_count += 1;
        }
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "sent_count": sent_count,
            "message": format!("Pengingat berhasil dikirim ke {sent_count} siswa")
        }),
        req_ctx.correlation_id.clone(),
    )))
}
