use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateQrTokenCommand {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub token_type: Option<String>,
    pub label: Option<String>,
    pub expires_in_days: Option<i64>,
    #[serde(default)]
    pub force_reset: Option<bool>,
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
        let now = self.clock.now();

        // If not explicit force_reset, reuse existing active token if raw_token is present
        if !command.force_reset.unwrap_or(false) {
            let existing = sqlx::query(
                r#"
                SELECT id, token_type, label, raw_token, expires_at, created_at
                FROM user_qr_tokens
                WHERE tenant_id = $1 AND user_id = $2 AND is_active = true
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            )
            .bind(command.tenant_id)
            .bind(command.user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e)))?;

            if let Some(row) = existing {
                if let Ok(Some(existing_raw)) = row.try_get::<Option<String>, _>("raw_token") {
                    if !existing_raw.trim().is_empty() {
                        return Ok(GeneratedQrToken {
                            id: row.get("id"),
                            raw_token: existing_raw,
                            user_id: command.user_id,
                            tenant_id: command.tenant_id,
                            token_type: row.get("token_type"),
                            label: row.get("label"),
                            expires_at: row.try_get("expires_at").unwrap_or(None),
                            created_at: row.get("created_at"),
                        });
                    }
                }
            }
        }

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

        // Deactivate previous active tokens for this user so old/lost cards are immediately revoked
        sqlx::query(
            r#"
            UPDATE user_qr_tokens
            SET is_active = false, updated_at = $3
            WHERE tenant_id = $1 AND user_id = $2 AND is_active = true
            "#,
        )
        .bind(command.tenant_id)
        .bind(command.user_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e)))?;

        sqlx::query(
            r#"
            INSERT INTO user_qr_tokens (
                id, tenant_id, user_id, token_hash, raw_token, token_type, label, is_active, expires_at, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, true, $8, $9, $9
            )
            "#,
        )
        .bind(token_id)
        .bind(command.tenant_id)
        .bind(command.user_id)
        .bind(&token_hash)
        .bind(&raw_token)
        .bind(&token_type)
        .bind(&label)
        .bind(expires_at)
        .bind(now)
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
