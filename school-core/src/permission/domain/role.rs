use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub is_system_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Role {
    pub fn new(tenant_id: Uuid, name: String, is_system_default: bool, clock: &dyn Clock) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            name,
            is_system_default,
            created_at: now,
            updated_at: now,
        }
    }
}
