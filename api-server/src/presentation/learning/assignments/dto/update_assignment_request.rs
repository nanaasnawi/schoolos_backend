use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateAssignmentRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub max_score: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
}
