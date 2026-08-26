use crate::common::error::ApplicationError;
use crate::learning::domain::lesson_plan::LessonPlan;
use crate::learning::infrastructure::repository_traits::LessonPlanRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateLessonPlanCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub teaching_methods: Option<String>,
    pub activities_opening: Option<String>,
    pub activities_core: Option<String>,
    pub activities_closing: Option<String>,
    pub resources: Option<String>,
    pub assessment_criteria: Option<String>,
}

pub struct CreateLessonPlanUseCase {
    plan_repo: Arc<dyn LessonPlanRepository>,
}

impl CreateLessonPlanUseCase {
    pub fn new(plan_repo: Arc<dyn LessonPlanRepository>) -> Self {
        Self { plan_repo }
    }

    pub async fn execute(
        &self,
        command: CreateLessonPlanCommand,
    ) -> Result<LessonPlan, ApplicationError> {
        let plan = LessonPlan::new(
            command.tenant_id,
            command.lesson_id,
            command.teaching_methods,
            command.activities_opening,
            command.activities_core,
            command.activities_closing,
            command.resources,
            command.assessment_criteria,
        );

        self.plan_repo.create(&plan).await?;

        Ok(plan)
    }
}
