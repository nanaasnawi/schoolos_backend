use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentInput {
    pub name: String,
    pub component_type: String,
    pub weight_percentage: f64,
    #[serde(default = "default_true")]
    pub is_required: bool,
    #[serde(default)]
    pub order_index: i32,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigureRulesRequest {
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub minimum_passing_grade: Option<f64>,
    pub components: Vec<ComponentInput>,
}
