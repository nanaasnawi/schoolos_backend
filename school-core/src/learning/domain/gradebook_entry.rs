use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradebookEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub component_id: Option<Uuid>,
    pub component_name: String,
    pub source_type: String,
    pub raw_score: Option<f64>,
    pub max_raw_score: Option<f64>,
    pub weighted_score: Option<f64>,
    pub weight_percentage: Option<f64>,
    pub source_id: Option<Uuid>,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl GradebookEntry {
    pub fn new(
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
        component_id: Option<Uuid>,
        component_name: String,
        source_type: String,
        raw_score: Option<f64>,
        max_raw_score: Option<f64>,
        weighted_score: Option<f64>,
        weight_percentage: Option<f64>,
        source_id: Option<Uuid>,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!student_id.is_nil(), "student_id must not be nil");

        Self {
            id: Uuid::now_v7(),
            tenant_id,
            student_id,
            class_id,
            subject_id,
            component_id,
            component_name,
            source_type,
            raw_score,
            max_raw_score,
            weighted_score,
            weight_percentage,
            source_id,
            calculated_at: Utc::now(),
            created_at: Utc::now(),
        }
    }
}
