use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::common::event_bus::SharedEventBus;
use crate::learning::domain::assessment_rule::AssessmentRule;
use crate::learning::infrastructure::repository_traits::AssessmentRuleRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ComponentInput {
    pub name: String,
    pub component_type: String,
    pub weight_percentage: f64,
    pub is_required: bool,
    pub order_index: i32,
}

pub struct ConfigureRulesCommand {
    pub tenant_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub minimum_passing_grade: Option<f64>,
    pub components: Vec<ComponentInput>,
}

pub struct ConfigureRulesUseCase {
    repo: Arc<dyn AssessmentRuleRepository>,
    clock: Arc<dyn Clock>,
    event_bus: SharedEventBus,
}

impl ConfigureRulesUseCase {
    pub fn new(
        repo: Arc<dyn AssessmentRuleRepository>,
        clock: Arc<dyn Clock>,
        event_bus: SharedEventBus,
    ) -> Self {
        Self {
            repo,
            clock,
            event_bus,
        }
    }

    pub async fn execute(
        &self,
        command: ConfigureRulesCommand,
    ) -> Result<AssessmentRule, ApplicationError> {
        let existing = self
            .repo
            .find_by_class_subject(command.class_id, command.subject_id)
            .await?;

        let mut rule = if let Some(mut r) = existing {
            r.components.clear();
            r
        } else {
            AssessmentRule::new(
                command.tenant_id,
                command.class_id,
                command.subject_id,
                command.academic_term_id,
                command.minimum_passing_grade.unwrap_or(70.0),
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?
        };

        for comp in command.components {
            rule.add_component(
                comp.name,
                comp.component_type,
                comp.weight_percentage,
                comp.is_required,
                comp.order_index,
            )
            .map_err(ApplicationError::Domain)?;
        }

        rule.activate(&*self.clock)
            .map_err(ApplicationError::Domain)?;

        self.repo.save(&rule).await?;

        for event in rule.take_events() {
            let _ = self.event_bus.publish(Arc::from(event)).await;
        }

        Ok(rule)
    }
}
