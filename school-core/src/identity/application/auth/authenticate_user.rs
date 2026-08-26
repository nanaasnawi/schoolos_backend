use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::identity::infrastructure::pg_user_repository::UserRepository;
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub exp: usize,
}

pub struct AuthenticateUserCommand {
    pub tenant_id: Uuid,
    pub email: String,
    pub password: String,
}

pub struct AuthenticateUserUseCase {
    user_repo: Arc<dyn UserRepository>,
    jwt_secret: String,
    clock: Arc<dyn Clock>,
}

impl AuthenticateUserUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        jwt_secret: String,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            user_repo,
            jwt_secret,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: AuthenticateUserCommand,
    ) -> Result<String, ApplicationError> {
        let user_opt = self
            .user_repo
            .find_by_email(command.tenant_id, &command.email)
            .await?;

        // If not found in the given tenant, fall back to a global email search.
        // This allows login without needing to send x-tenant-id header.
        let user_opt = if user_opt.is_none() {
            self.user_repo
                .find_by_email_global(&command.email)
                .await?
        } else {
            user_opt
        };

        if let Some(user) = user_opt {
            let parsed_hash_result = PasswordHash::new(&user.password_hash);
            let is_valid = match parsed_hash_result {
                Ok(parsed_hash) => Argon2::default().verify_password(command.password.as_bytes(), &parsed_hash).is_ok(),
                Err(_) => false,
            };
            if is_valid {
                let expiration = self
                    .clock
                    .now()
                    .checked_add_signed(chrono::Duration::hours(24))
                    .expect("valid timestamp")
                    .timestamp() as usize;

                let claims = Claims {
                    sub: user.id.to_string(),
                    tenant_id: user.tenant_id.to_string(),
                    email: Some(user.email.clone()),
                    full_name: Some(user.full_name.clone()),
                    role: Some("Administrator".to_string()),
                    exp: expiration,
                };

                let token = encode(
                    &Header::default(),
                    &claims,
                    &EncodingKey::from_secret(self.jwt_secret.as_ref()),
                )
                .map_err(|e| {
                    ApplicationError::Internal(format!("Failed to encode token: {}", e))
                })?;
                return Ok(token);
            }
        }

        Err(ApplicationError::Unauthorized(
            ErrorCode::AuthInvalidCredentials,
            "Invalid credentials".to_string(),
        ))
    }
}
