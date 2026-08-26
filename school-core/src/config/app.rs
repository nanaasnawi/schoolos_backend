use super::{
    database::DatabaseConfig,
    features::FeatureFlags,
    logging::LoggingConfig,
    security::{JwtConfig, SecurityConfig},
    server::ServerConfig,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub jwt: JwtConfig,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub features: FeatureFlags,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        let _ = dotenvy::dotenv();

        config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}
