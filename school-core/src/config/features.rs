use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct FeatureFlags {
    pub enable_new_dashboard: bool,
}
