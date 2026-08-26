use chrono::{DateTime, Utc};
use school_core::learning::domain::gradebook::{GradeBook, GradeEntry};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct GradeEntryResponse {
    pub id: Uuid,
    pub gradebook_id: Uuid,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub component_name: String,
    pub raw_score: f64,
    pub max_raw_score: f64,
    pub weight_percentage: f64,
    pub weighted_score: f64,
    pub recorded_at: DateTime<Utc>,
}

impl From<GradeEntry> for GradeEntryResponse {
    fn from(e: GradeEntry) -> Self {
        Self {
            id: e.id,
            gradebook_id: e.gradebook_id,
            source_type: e.source_type,
            source_id: e.source_id,
            component_name: e.component_name,
            raw_score: e.raw_score,
            max_raw_score: e.max_raw_score,
            weight_percentage: e.weight_percentage,
            weighted_score: e.weighted_score,
            recorded_at: e.recorded_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GradeBookResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub final_score: Option<f64>,
    pub letter_grade: Option<String>,
    pub passed: Option<bool>,
    pub status: String,
    pub entries: Vec<GradeEntryResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GradeBook> for GradeBookResponse {
    fn from(gb: GradeBook) -> Self {
        Self {
            id: gb.id,
            tenant_id: gb.tenant_id,
            student_id: gb.student_id,
            class_id: gb.class_id,
            subject_id: gb.subject_id,
            academic_year_id: gb.academic_year_id,
            final_score: gb.final_score,
            letter_grade: gb.letter_grade,
            passed: gb.passed,
            status: gb.status,
            entries: gb
                .entries
                .into_iter()
                .map(GradeEntryResponse::from)
                .collect(),
            created_at: gb.created_at,
            updated_at: gb.updated_at,
        }
    }
}
