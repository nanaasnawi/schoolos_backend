use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UpdateLearningMaterialRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub visibility: Option<String>,
    pub start_page: Option<i32>,
    pub end_page: Option<i32>,
    pub release_at: Option<DateTime<Utc>>,
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
}
