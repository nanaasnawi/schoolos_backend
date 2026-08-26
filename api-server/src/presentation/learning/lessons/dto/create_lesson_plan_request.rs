use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLessonPlanRequest {
    pub teaching_methods: Option<String>,
    pub activities_opening: Option<String>,
    pub activities_core: Option<String>,
    pub activities_closing: Option<String>,
    pub resources: Option<String>,
    pub assessment_criteria: Option<String>,
}
