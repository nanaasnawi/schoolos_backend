use chrono::{DateTime, Utc};
use school_core::learning::domain::gradebook_entry::GradebookEntry;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
pub struct GradebookEntryResponse {
    pub id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub component_name: String,
    pub source_type: String,
    pub raw_score: Option<f64>,
    pub max_raw_score: Option<f64>,
    pub weighted_score: Option<f64>,
    pub weight_percentage: Option<f64>,
    pub calculated_at: DateTime<Utc>,
}

impl From<GradebookEntry> for GradebookEntryResponse {
    fn from(e: GradebookEntry) -> Self {
        Self {
            id: e.id,
            student_id: e.student_id,
            class_id: e.class_id,
            subject_id: e.subject_id,
            component_name: e.component_name,
            source_type: e.source_type,
            raw_score: e.raw_score,
            max_raw_score: e.max_raw_score,
            weighted_score: e.weighted_score,
            weight_percentage: e.weight_percentage,
            calculated_at: e.calculated_at,
        }
    }
}
