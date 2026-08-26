use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct School {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub npsn: Option<String>,
    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub logo_url: Option<String>,
    pub status: String,
    pub accreditation: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,
}

impl School {
    pub fn new(tenant_id: Uuid, name: String, clock: &dyn Clock) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            name,
            npsn: None,
            address: None,
            phone_number: None,
            email: None,
            logo_url: None,
            status: "Active".to_string(),
            accreditation: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
        }
    }
}
