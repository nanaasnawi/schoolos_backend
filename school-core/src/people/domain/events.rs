use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::domain::event::{DomainEvent, EventMetadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentCreatedEvent {
    pub metadata: EventMetadata,
    pub student_id: Uuid,
    pub nisn: Option<String>,
    pub nipd: Option<String>,
}

impl DomainEvent for StudentCreatedEvent {
    fn event_name(&self) -> &'static str {
        "StudentCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentUpdatedEvent {
    pub metadata: EventMetadata,
    pub student_id: Uuid,
}

impl DomainEvent for StudentUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "StudentUpdated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Teacher Events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherCreatedEvent {
    pub metadata: EventMetadata,
    pub teacher_id: Uuid,
    pub nip: Option<String>,
    pub full_name: String,
}

impl DomainEvent for TeacherCreatedEvent {
    fn event_name(&self) -> &'static str {
        "TeacherCreated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherUpdatedEvent {
    pub metadata: EventMetadata,
    pub teacher_id: Uuid,
}

impl DomainEvent for TeacherUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "TeacherUpdated"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Guardian Events ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianCreatedEvent {
    pub metadata: EventMetadata,
    pub guardian_id: Uuid,
    pub full_name: String,
}

impl DomainEvent for GuardianCreatedEvent {
    fn event_name(&self) -> &'static str {
        "GuardianCreated"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianUpdatedEvent {
    pub metadata: EventMetadata,
    pub guardian_id: Uuid,
}

impl DomainEvent for GuardianUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "GuardianUpdated"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ─── Staff Events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffCreatedEvent {
    pub metadata: EventMetadata,
    pub staff_id: Uuid,
    pub full_name: String,
}

impl DomainEvent for StaffCreatedEvent {
    fn event_name(&self) -> &'static str {
        "StaffCreated"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffUpdatedEvent {
    pub metadata: EventMetadata,
    pub staff_id: Uuid,
}

impl DomainEvent for StaffUpdatedEvent {
    fn event_name(&self) -> &'static str {
        "StaffUpdated"
    }
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
