use school_core::learning::domain::assessment_rule::AssessmentRule;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentResponse {
    pub id: Uuid,
    pub name: String,
    pub component_type: String,
    pub weight_percentage: f64,
    pub is_required: bool,
    pub order_index: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssessmentRuleResponse {
    pub id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub minimum_passing_grade: f64,
    pub status: String,
    pub rounding_policy: String,
    pub is_active: bool,
    pub components: Vec<ComponentResponse>,
}

impl From<AssessmentRule> for AssessmentRuleResponse {
    fn from(r: AssessmentRule) -> Self {
        Self {
            id: r.id,
            class_id: r.class_id,
            subject_id: r.subject_id,
            academic_term_id: r.academic_term_id,
            minimum_passing_grade: r.minimum_passing_grade,
            status: r.status,
            rounding_policy: r.rounding_policy,
            is_active: r.is_active,
            components: r
                .components
                .into_iter()
                .map(|c| ComponentResponse {
                    id: c.id,
                    name: c.name,
                    component_type: c.component_type,
                    weight_percentage: c.weight_percentage,
                    is_required: c.is_required,
                    order_index: c.order_index,
                })
                .collect(),
        }
    }
}
