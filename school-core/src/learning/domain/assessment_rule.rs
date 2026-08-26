use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::common::error::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::{AssessmentRuleActivatedEvent, AssessmentRuleCreatedEvent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleStatus {
    Draft,
    Active,
    Archived,
}

impl RuleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeComponent {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub name: String,
    pub component_type: String,
    pub weight_percentage: f64,
    pub is_required: bool,
    pub order_index: i32,
}

pub type AssessmentComponent = GradeComponent;

impl GradeComponent {
    pub fn new(
        rule_id: Uuid,
        name: String,
        component_type: String,
        weight_percentage: f64,
        is_required: bool,
        order_index: i32,
    ) -> Result<Self, DomainError> {
        if rule_id.is_nil() {
            return Err(DomainError::Validation(
                "rule_id must not be nil".to_string(),
            ));
        }
        if name.trim().is_empty() {
            return Err(DomainError::Validation(
                "component name must not be empty".to_string(),
            ));
        }
        if weight_percentage <= 0.0 {
            return Err(DomainError::Validation(
                "weight_percentage must be positive".to_string(),
            ));
        }

        Ok(Self {
            id: Uuid::now_v7(),
            rule_id,
            name,
            component_type,
            weight_percentage,
            is_required,
            order_index,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssessmentRule {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub minimum_passing_grade: f64,
    pub status: String,
    pub rounding_policy: String,
    pub is_active: bool,
    pub components: Vec<GradeComponent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for AssessmentRule {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            class_id: self.class_id,
            subject_id: self.subject_id,
            academic_term_id: self.academic_term_id,
            minimum_passing_grade: self.minimum_passing_grade,
            status: self.status.clone(),
            rounding_policy: self.rounding_policy.clone(),
            is_active: self.is_active,
            components: self.components.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for AssessmentRule {
    fn id(&self) -> Uuid {
        self.id
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.domain_events)
    }
}

impl AssessmentRule {
    pub fn new(
        tenant_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
        academic_term_id: Option<Uuid>,
        minimum_passing_grade: f64,
        clock: &dyn Clock,
    ) -> Result<Self, DomainError> {
        if tenant_id.is_nil() {
            return Err(DomainError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if class_id.is_nil() {
            return Err(DomainError::Validation(
                "class_id must not be nil".to_string(),
            ));
        }
        if subject_id.is_nil() {
            return Err(DomainError::Validation(
                "subject_id must not be nil".to_string(),
            ));
        }
        if !(0.0..=100.0).contains(&minimum_passing_grade) {
            return Err(DomainError::Validation(
                "minimum_passing_grade must be between 0.0 and 100.0".to_string(),
            ));
        }

        let now = clock.now();
        let id = Uuid::now_v7();

        let mut rule = Self {
            id,
            tenant_id,
            class_id,
            subject_id,
            academic_term_id,
            minimum_passing_grade,
            status: "draft".to_string(),
            rounding_policy: "half_up".to_string(),
            is_active: false,
            components: Vec::new(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        rule.raise_event(AssessmentRuleCreatedEvent {
            metadata: EventMetadata::new(
                "AssessmentRuleCreated".to_string(),
                tenant_id,
                id.to_string(),
                None,
                None,
                None,
                1,
                clock,
            ),
            rule_id: id,
            class_id,
            subject_id,
        });

        Ok(rule)
    }

    /// Domain Business Invariant: Add Grade Component
    /// Rules:
    /// - Weight percentage must be > 0.0
    /// - Cannot add two components with the exact same component_type
    /// - Cannot modify if archived
    pub fn add_component(
        &mut self,
        name: String,
        component_type: String,
        weight_percentage: f64,
        is_required: bool,
        order_index: i32,
    ) -> Result<(), DomainError> {
        if self.status == "archived" {
            return Err(DomainError::Validation(
                "Cannot modify an archived assessment rule".to_string(),
            ));
        }

        if self
            .components
            .iter()
            .any(|c| c.component_type.eq_ignore_ascii_case(&component_type))
        {
            return Err(DomainError::Validation(format!(
                "Component with type '{}' already exists in this assessment rule",
                component_type
            )));
        }

        let component = GradeComponent::new(
            self.id,
            name,
            component_type,
            weight_percentage,
            is_required,
            order_index,
        )?;

        self.components.push(component);
        Ok(())
    }

    /// Domain Business Invariant: Activate Assessment Rule
    /// Critical Rule: Sum of all component weights MUST equal 100.0%
    pub fn activate(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        if self.components.is_empty() {
            return Err(DomainError::Validation(
                "Cannot activate assessment rule without grade components".to_string(),
            ));
        }

        let total_weight: f64 = self.components.iter().map(|c| c.weight_percentage).sum();
        if (total_weight - 100.0).abs() > 0.01 {
            return Err(DomainError::Validation(format!(
                "Total component weights must equal 100.0%, got {:.2}%",
                total_weight
            )));
        }

        self.status = "active".to_string();
        self.is_active = true;
        self.updated_at = clock.now();

        self.raise_event(AssessmentRuleActivatedEvent {
            metadata: EventMetadata::new(
                "AssessmentRuleActivated".to_string(),
                self.tenant_id,
                self.id.to_string(),
                None,
                None,
                None,
                (self.version + 1) as u32,
                clock,
            ),
            rule_id: self.id,
            class_id: self.class_id,
            subject_id: self.subject_id,
        });

        Ok(())
    }

    pub fn archive(&mut self, clock: &dyn Clock) -> Result<(), DomainError> {
        self.status = "archived".to_string();
        self.is_active = false;
        self.updated_at = clock.now();
        Ok(())
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
