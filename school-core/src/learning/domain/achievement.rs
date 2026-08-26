use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::{DomainEvent, EventMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Domain Events ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementCreated {
    pub achievement_id: Uuid,
    pub title: String,
    pub tenant_id: Uuid,
    pub metadata: EventMetadata,
}

impl DomainEvent for AchievementCreated {
    fn event_name(&self) -> &str {
        "learning.achievement.created"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementEarned {
    pub student_achievement_id: Uuid,
    pub student_id: Uuid,
    pub achievement_id: Uuid,
    pub achievement_title: String,
    pub tenant_id: Uuid,
    pub metadata: EventMetadata,
}

impl DomainEvent for AchievementEarned {
    fn event_name(&self) -> &str {
        "learning.achievement.earned"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ── Child Entity ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentAchievement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub achievement_id: Uuid,
    pub earned_at: DateTime<Utc>,
}

impl StudentAchievement {
    pub fn new(tenant_id: Uuid, student_id: Uuid, achievement_id: Uuid) -> Self {
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            student_id,
            achievement_id,
            earned_at: Utc::now(),
        }
    }
}

// ── Aggregate ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Achievement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub criteria_type: String,
    pub criteria_value: String,
    pub is_published: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Achievement {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            title: self.title.clone(),
            description: self.description.clone(),
            icon: self.icon.clone(),
            criteria_type: self.criteria_type.clone(),
            criteria_value: self.criteria_value.clone(),
            is_published: self.is_published,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for Achievement {
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

impl Achievement {
    pub fn new(
        tenant_id: Uuid,
        title: String,
        description: String,
        icon: String,
        criteria_type: String,
        criteria_value: String,
        clock: &dyn Clock,
    ) -> Self {
        assert!(!tenant_id.is_nil(), "tenant_id must not be nil");
        assert!(!title.is_empty(), "title must not be empty");
        assert!(!criteria_type.is_empty(), "criteria_type must not be empty");

        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            title,
            description,
            icon,
            criteria_type,
            criteria_value,
            is_published: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn publish(&mut self) {
        self.is_published = true;
        self.updated_at = Utc::now();
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }
}
