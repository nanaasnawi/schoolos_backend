use serde::{Deserialize, Serialize};
use crate::common::domain::event::{DomainEvent, EventMetadata};
use crate::integration::contracts::{TeacherSyncRecord, ClassSyncRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherImportedEvent {
    pub metadata: EventMetadata,
    pub record: TeacherSyncRecord,
}

impl DomainEvent for TeacherImportedEvent {
    fn event_name(&self) -> &'static str {
        "TeacherImportedFromDapodik"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassImportedEvent {
    pub metadata: EventMetadata,
    pub record: ClassSyncRecord,
}

impl DomainEvent for ClassImportedEvent {
    fn event_name(&self) -> &'static str {
        "ClassImportedFromDapodik"
    }

    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
