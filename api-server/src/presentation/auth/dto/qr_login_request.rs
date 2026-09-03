use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct QrLoginRequest {
    /// Token string yang terbaca dari QR Code
    #[schema(example = "sch_qr_v1_018f9a2b4c...")]
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct GenerateQrBadgeRequest {
    /// User ID yang akan dibuatkan kartu badge QR
    pub user_id: uuid::Uuid,
    /// Tipe token: 'BADGE' (kartu fisik/reusable) atau 'ONE_TIME'
    pub token_type: Option<String>,
    /// Label kartu, misal 'Kartu Pelajar 2026/2027'
    pub label: Option<String>,
    /// Masa berlaku dalam hari (opsional, default: aktif terus sampai di-revoke)
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct BatchGenerateQrBadgesRequest {
    /// Daftar User ID yang akan dibuatkan kartu badge QR
    pub user_ids: Vec<uuid::Uuid>,
    /// Tipe token: 'BADGE' (default)
    pub token_type: Option<String>,
    /// Label kartu, misal 'Kartu Akses Mobile 2026'
    pub label: Option<String>,
    /// Masa berlaku dalam hari (opsional)
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserQrStatusDto {
    pub id: uuid::Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub identifier: Option<String>,
    pub class_name: Option<String>,
    pub has_active_token: bool,
    pub active_token_label: Option<String>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub token_created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub token_last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchGenerateQrItemDto {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub raw_token: String,
    pub full_name: String,
    pub email: String,
    pub role: String,
    pub identifier: Option<String>,
    pub class_name: Option<String>,
    pub token_type: String,
    pub label: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

