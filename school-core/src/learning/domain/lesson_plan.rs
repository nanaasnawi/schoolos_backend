use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct LessonPlan {
    pub id: Uuid,
    pub tenant_id: Uuid,
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
    pub deleted_at: Option<DateTime<Utc>>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
}

impl Clone for LessonPlan {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            lesson_id: self.lesson_id,
            teaching_methods: self.teaching_methods.clone(),
            activities_opening: self.activities_opening.clone(),
            activities_core: self.activities_core.clone(),
            activities_closing: self.activities_closing.clone(),
            resources: self.resources.clone(),
            assessment_criteria: self.assessment_criteria.clone(),
            version: self.version,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            domain_events: Vec::new(),
        }
    }
}

impl LessonPlan {
    pub fn new(
        tenant_id: Uuid,
        lesson_id: Uuid,
        teaching_methods: Option<String>,
        activities_opening: Option<String>,
        activities_core: Option<String>,
        activities_closing: Option<String>,
        resources: Option<String>,
        assessment_criteria: Option<String>,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!lesson_id.is_nil(), "lesson_id must not be nil");

        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            lesson_id,
            teaching_methods,
            activities_opening,
            activities_core,
            activities_closing,
            resources,
            assessment_criteria,
            version: 1,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            domain_events: Vec::new(),
        }
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        lesson_id: Uuid,
        teaching_methods: Option<String>,
        activities_opening: Option<String>,
        activities_core: Option<String>,
        activities_closing: Option<String>,
        resources: Option<String>,
        assessment_criteria: Option<String>,
        version: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            lesson_id,
            teaching_methods,
            activities_opening,
            activities_core,
            activities_closing,
            resources,
            assessment_criteria,
            version,
            created_at,
            updated_at,
            deleted_at,
            domain_events: Vec::new(),
        }
    }
}
