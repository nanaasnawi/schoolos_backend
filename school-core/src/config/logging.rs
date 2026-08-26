use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
}
