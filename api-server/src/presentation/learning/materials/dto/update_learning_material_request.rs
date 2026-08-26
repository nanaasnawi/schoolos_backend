use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateLearningMaterialRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub visibility: Option<String>,
}
