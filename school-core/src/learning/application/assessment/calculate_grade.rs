use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::final_grade_calculator::FinalGradeCalculator;
use crate::learning::domain::gradebook::GradeBook;
use crate::learning::infrastructure::repository_traits::{
    AssessmentRuleRepository, GradebookRepository,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct ComponentScoreInput {
    pub component_name: String,
    pub source_type: String,
    pub raw_score: f64,
    pub max_raw_score: f64,
    pub source_id: Option<Uuid>,
}

pub struct CalculateGradeCommand {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub scores: Vec<ComponentScoreInput>,
}

pub struct CalculateGradeUseCase {
    rule_repo: Arc<dyn AssessmentRuleRepository>,
    gradebook_repo: Arc<dyn GradebookRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl CalculateGradeUseCase {
    pub fn new(
        rule_repo: Arc<dyn AssessmentRuleRepository>,
        gradebook_repo: Arc<dyn GradebookRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            rule_repo,
            gradebook_repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: CalculateGradeCommand,
    ) -> Result<GradeBook, ApplicationError> {
        let rule = self
            .rule_repo
            .find_by_class_subject(command.class_id, command.subject_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::AssessmentRulesNotFound,
                    format!(
                        "No assessment rule found for class {} and subject {}",
                        command.class_id, command.subject_id
                    ),
                )
            })?;

        let existing_gb = self
            .gradebook_repo
            .find_gradebook_by_student_subject(
                command.student_id,
                command.class_id,
                command.subject_id,
            )
            .await?;

        let mut gradebook = if let Some(gb) = existing_gb {
            gb
        } else {
            GradeBook::new(
                command.tenant_id,
                command.student_id,
                command.class_id,
                command.subject_id,
                command.academic_year_id,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?
        };

        for score in command.scores {
            let weight_percentage = rule
                .components
                .iter()
                .find(|c| {
                    c.name.eq_ignore_ascii_case(&score.component_name)
                        || c.component_type.eq_ignore_ascii_case(&score.source_type)
                })
                .map(|c| c.weight_percentage)
                .unwrap_or(0.0);

            gradebook
                .record_grade(
                    score.source_type,
                    score.source_id,
                    score.component_name,
                    score.raw_score,
                    score.max_raw_score,
                    weight_percentage,
                    &*self.clock,
                )
                .map_err(ApplicationError::Domain)?;
        }

        FinalGradeCalculator::calculate(&rule, &mut gradebook, &*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.gradebook_repo.save_gradebook(&gradebook).await?;

        for event in gradebook.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(gradebook)
    }
}
