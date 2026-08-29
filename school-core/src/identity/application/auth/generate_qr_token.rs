use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateQrTokenCommand {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub token_type: Option<String>,
    pub label: Option<String>,
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedQrToken {
    pub id: Uuid,
    pub raw_token: String,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub token_type: String,
    pub label: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct GenerateQrTokenUseCase {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl GenerateQrTokenUseCase {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    pub async fn execute(
        &self,
        command: GenerateQrTokenCommand,
    ) -> Result<GeneratedQrToken, ApplicationError> {
        let token_id = Uuid::now_v7();
        let entropy = Uuid::now_v7().to_string().replace('-', "");
        let raw_token = format!("sch_qr_v1_{}_{}", token_id.to_string().replace('-', ""), &entropy[0..16]);

        let mut hasher = Sha256::new();
        hasher.update(raw_token.as_bytes());
        let token_hash = hasher.finalize().encode_hex::<String>();

        let token_type = command.token_type.unwrap_or_else(|| "BADGE".to_string());
        let label = command
            .label
            .unwrap_or_else(|| "Kartu Identitas Digital".to_string());

        let expires_at = command.expires_in_days.map(|days| {
            self.clock
                .now()
                .checked_add_signed(chrono::Duration::days(days))
                .unwrap_or_else(|| self.clock.now())
        });

        let now = self.clock.now();

        sqlx::query!(
            r#"
            INSERT INTO user_qr_tokens (
                id, tenant_id, user_id, token_hash, token_type, label, is_active, expires_at, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, true, $7, $8, $8
            )
            "#,
            token_id,
            command.tenant_id,
            command.user_id,
            token_hash,
            token_type,
            label,
            expires_at,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e)))?;

        Ok(GeneratedQrToken {
            id: token_id,
            raw_token,
            user_id: command.user_id,
            tenant_id: command.tenant_id,
            token_type,
            label,
            expires_at,
            created_at: now,
        })
    }
}
