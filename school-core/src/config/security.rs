use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub password_salt_len: usize,
    pub token_expiration_hours: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
}
