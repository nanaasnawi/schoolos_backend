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
