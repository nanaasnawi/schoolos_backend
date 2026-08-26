use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::lesson_plan::LessonPlan;
use crate::learning::infrastructure::repository_traits::LessonPlanRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetLessonPlanQuery {
    pub lesson_id: Uuid,
}

pub struct GetLessonPlanUseCase {
    plan_repo: Arc<dyn LessonPlanRepository>,
}

impl GetLessonPlanUseCase {
    pub fn new(plan_repo: Arc<dyn LessonPlanRepository>) -> Self {
        Self { plan_repo }
    }

    pub async fn execute(&self, query: GetLessonPlanQuery) -> Result<LessonPlan, ApplicationError> {
        self.plan_repo
            .find_by_lesson_id(query.lesson_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::LessonPlanNotFound,
                    format!("LessonPlan for lesson {} not found", query.lesson_id),
                )
            })
    }
}
