use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::assessment_rule::AssessmentRule;
use crate::learning::infrastructure::repository_traits::AssessmentRuleRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetRulesQuery {
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

pub struct GetRulesUseCase {
    repo: Arc<dyn AssessmentRuleRepository>,
}

impl GetRulesUseCase {
    pub fn new(repo: Arc<dyn AssessmentRuleRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: GetRulesQuery) -> Result<AssessmentRule, ApplicationError> {
        self.repo
            .find_by_class_subject(query.class_id, query.subject_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssessmentRulesNotFound,
                    format!(
                        "Assessment rules for class {} subject {} not found",
                        query.class_id, query.subject_id
                    ),
                )
            })
    }
}
