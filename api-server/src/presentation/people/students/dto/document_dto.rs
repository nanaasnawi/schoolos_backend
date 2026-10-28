use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DocumentItemDto {
    pub id: Option<Uuid>,
    pub document_type: String, // 'KK', 'AKTA', 'IJAZAH_SD', 'IJAZAH_SMP', 'KTP', 'PAS_FOTO', 'KIP'
    pub label: String,
    pub description: String,
    pub is_required: bool,
    pub status: String, // 'UNUPLOADED', 'PENDING', 'VERIFIED', 'REJECTED'
    pub file_url: Option<String>,
    pub file_name: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub notes: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StudentDossierOverviewResponse {
    pub student_id: Uuid,
    pub nisn: String,
    pub full_name: String,
    pub class_name: Option<String>,
    pub program_tier: String, // 'PAKET_A', 'PAKET_B', 'PAKET_C', 'REGULER'
    pub program_tier_label: String,
    pub is_complete: bool,
    pub completeness_percentage: f64,
    pub required_count: usize,
    pub verified_count: usize,
    pub pending_count: usize,
    pub rejected_count: usize,
    pub documents: Vec<DocumentItemDto>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct VerifyDocumentPayload {
    pub status: String, // 'VERIFIED' or 'REJECTED'
    pub rejection_reason: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StudentDocumentSummaryRow {
    pub student_id: Uuid,
    pub nisn: String,
    pub full_name: String,
    pub class_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub program_tier: String,
    pub is_complete: bool,
    pub completeness_percentage: f64,
    pub formulir_status: String,
    pub kk_status: String,
    pub akta_status: String,
    pub ijazah_status: String,
    pub ktp_status: String,
    pub foto_status: String,
    pub kip_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClassDossierStat {
    pub class_id: Option<Uuid>,
    pub class_name: String,
    pub total: i64,
    pub complete: i64,
    pub incomplete: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RekapRombelDossierResponse {
    pub total_students: i64,
    pub complete_students: i64,
    pub incomplete_students: i64,
    pub completeness_percentage: f64,
    pub classes_summary: Vec<ClassDossierStat>,
    pub students: Vec<StudentDocumentSummaryRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RekapQuery {
    pub class_id: Option<Uuid>,
    pub program_tier: Option<String>, // 'PAKET_A', 'PAKET_B', 'PAKET_C'
    pub status_filter: Option<String>, // 'COMPLETE', 'INCOMPLETE'
    pub search: Option<String>,
}
