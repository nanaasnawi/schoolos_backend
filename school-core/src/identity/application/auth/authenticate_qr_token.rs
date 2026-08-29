use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::identity::application::auth::authenticate_user::Claims;
use hex::ToHex;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrAuthResult {
    pub token: String,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub expires_in: usize,
}

pub struct AuthenticateQrTokenUseCase {
    pool: PgPool,
    jwt_secret: String,
    clock: Arc<dyn Clock>,
}

impl AuthenticateQrTokenUseCase {
    pub fn new(pool: PgPool, jwt_secret: String, clock: Arc<dyn Clock>) -> Self {
        Self {
            pool,
            jwt_secret,
            clock,
        }
    }

    pub async fn execute(&self, raw_token: &str) -> Result<QrAuthResult, ApplicationError> {
        let trimmed_token = raw_token.trim();
        if trimmed_token.is_empty() {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthInvalidCredentials,
                "QR Token tidak boleh kosong.".to_string(),
            ));
        }

        // 1. Calculate SHA-256 hash of the raw token
        let mut hasher = Sha256::new();
        hasher.update(trimmed_token.as_bytes());
        let token_hash = hasher.finalize().encode_hex::<String>();

        // 2. Query user_qr_tokens table with user and role details
        let record = sqlx::query!(
            r#"
            SELECT 
                t.id as token_id,
                t.token_type,
                t.is_active as token_is_active,
                t.expires_at,
                u.id as user_id,
                u.tenant_id,
                u.email,
                u.full_name,
                u.is_active as user_is_active,
                COALESCE(r.name, 'Siswa') as "role_name!"
            FROM user_qr_tokens t
            JOIN users u ON t.user_id = u.id
            LEFT JOIN user_roles ur ON u.id = ur.user_id
            LEFT JOIN roles r ON ur.role_id = r.id
            WHERE t.token_hash = $1
            LIMIT 1
            "#,
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e)))?;

        let record = match record {
            Some(r) => r,
            None => {
                return Err(ApplicationError::Unauthorized(
                    ErrorCode::AuthInvalidCredentials,
                    "QR Code tidak valid atau belum terdaftar di sistem.".to_string(),
                ));
            }
        };

        // 3. Validation: Active & Expiration
        if !record.token_is_active {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthInvalidCredentials,
                "Kartu / QR Code ini telah dinonaktifkan oleh administrator.".to_string(),
            ));
        }

        if !record.user_is_active {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthInvalidCredentials,
                "Akun pengguna yang terkait QR ini sedang tidak aktif.".to_string(),
            ));
        }

        let now = self.clock.now();
        if let Some(expires_at) = record.expires_at {
            if expires_at < now {
                return Err(ApplicationError::Unauthorized(
                    ErrorCode::AuthInvalidCredentials,
                    "Masa berlaku QR Code telah kadaluarsa. Silakan minta kartu baru ke pihak sekolah.".to_string(),
                ));
            }
        }

        // 4. Update last_used_at and handle ONE_TIME token
        if record.token_type.eq_ignore_ascii_case("ONE_TIME") {
            let _ = sqlx::query!(
                "UPDATE user_qr_tokens SET last_used_at = NOW(), is_active = false WHERE id = $1",
                record.token_id
            )
            .execute(&self.pool)
            .await;
        } else {
            let _ = sqlx::query!(
                "UPDATE user_qr_tokens SET last_used_at = NOW() WHERE id = $1",
                record.token_id
            )
            .execute(&self.pool)
            .await;
        }

        // 5. Generate JWT Access Token
        let expiration = now
            .checked_add_signed(chrono::Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = Claims {
            sub: record.user_id.to_string(),
            tenant_id: record.tenant_id.to_string(),
            email: Some(record.email.clone()),
            full_name: Some(record.full_name.clone()),
            role: Some(record.role_name.clone()),
            exp: expiration,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| ApplicationError::Internal(format!("Failed to encode token: {}", e)))?;

        Ok(QrAuthResult {
            token,
            user_id: record.user_id,
            tenant_id: record.tenant_id,
            email: record.email,
            full_name: record.full_name,
            role: record.role_name,
            expires_in: 86400,
        })
    }
}
