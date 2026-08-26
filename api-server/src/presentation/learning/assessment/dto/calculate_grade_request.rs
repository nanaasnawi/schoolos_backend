use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentScoreInput {
    pub component_name: String,
    pub source_type: String,
    pub raw_score: f64,
    pub max_raw_score: f64,
    pub source_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CalculateGradeRequest {
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub scores: Vec<ComponentScoreInput>,
}
