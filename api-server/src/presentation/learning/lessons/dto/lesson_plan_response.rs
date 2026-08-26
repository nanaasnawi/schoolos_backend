use chrono::{DateTime, Utc};
use school_core::learning::domain::lesson_plan::LessonPlan;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonPlanResponse {
    pub id: Uuid,
    pub lesson_id: Uuid,
    pub teaching_methods: Option<String>,
    pub activities_opening: Option<String>,
    pub activities_core: Option<String>,
    pub activities_closing: Option<String>,
    pub resources: Option<String>,
    pub assessment_criteria: Option<String>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<LessonPlan> for LessonPlanResponse {
    fn from(p: LessonPlan) -> Self {
        Self {
            id: p.id,
            lesson_id: p.lesson_id,
            teaching_methods: p.teaching_methods,
            activities_opening: p.activities_opening,
            activities_core: p.activities_core,
            activities_closing: p.activities_closing,
            resources: p.resources,
            assessment_criteria: p.assessment_criteria,
            version: p.version,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}
